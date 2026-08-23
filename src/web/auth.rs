use crate::config::Config;
use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::BTreeMap;
use tracing::warn;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedIdentity {
    pub auth_type: String,
    pub user_identifier: String,
}

/// Verifies Telegram Mini App `initData` query string using HMAC-SHA256.
///
/// Specification:
/// 1. Parse all key=value pairs, separate `hash`.
/// 2. Sort key-value pairs alphabetically and format as `key=value\n...` (data-check-string).
/// 3. `secret_key = HMAC_SHA256("WebAppData", bot_token)`
/// 4. `calculated_hash = hex(HMAC_SHA256(secret_key, data-check-string))`
/// 5. Compare `calculated_hash` with `hash`.
pub fn verify_telegram_init_data(
    init_data_raw: &str,
    bot_token: &str,
    expected_chat_id: Option<i64>,
) -> Result<AuthenticatedIdentity, &'static str> {
    if init_data_raw.is_empty() || bot_token.is_empty() {
        return Err("Empty init_data or bot_token");
    }

    let mut pairs: BTreeMap<String, String> = BTreeMap::new();
    let mut provided_hash = String::new();

    for part in init_data_raw.split('&') {
        if let Some((k, v)) = part.split_once('=') {
            let key = urlencoding_decode(k);
            let val = urlencoding_decode(v);
            if key == "hash" {
                provided_hash = val;
            } else {
                pairs.insert(key, val);
            }
        }
    }

    if provided_hash.is_empty() {
        return Err("Missing hash in Telegram initData");
    }

    // Check auth_date for replay attack protection (max 24h)
    if let Some(auth_date_str) = pairs.get("auth_date") {
        if let Ok(auth_date) = auth_date_str.parse::<i64>() {
            let now = chrono::Utc::now().timestamp();
            if (now - auth_date).abs() > 86400 {
                return Err("Telegram initData expired (>24h)");
            }
        }
    }

    // Build data-check-string
    let mut check_str = String::new();
    for (i, (k, v)) in pairs.iter().enumerate() {
        if i > 0 {
            check_str.push('\n');
        }
        check_str.push_str(k);
        check_str.push('=');
        check_str.push_str(v);
    }

    // 1. secret_key = HMAC_SHA256("WebAppData", bot_token)
    let mut mac_secret = match HmacSha256::new_from_slice(b"WebAppData") {
        Ok(m) => m,
        Err(_) => return Err("Failed to create HMAC for secret key"),
    };
    mac_secret.update(bot_token.as_bytes());
    let secret_key = mac_secret.finalize().into_bytes();

    // 2. data_hash = HMAC_SHA256(secret_key, data_check_string)
    let mut mac_data = match HmacSha256::new_from_slice(&secret_key) {
        Ok(m) => m,
        Err(_) => return Err("Failed to create HMAC for data check"),
    };
    mac_data.update(check_str.as_bytes());
    let data_hash = hex::encode(mac_data.finalize().into_bytes());

    if !constant_time_eq(&data_hash, &provided_hash) {
        return Err("Invalid Telegram hash signature");
    }

    // Validate User ID against configured chat_id (if chat_id is set)
    let user_json_opt = pairs.get("user");
    let mut user_id_str = "unknown".to_string();

    if let Some(user_json) = user_json_opt {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(user_json) {
            if let Some(id) = v.get("id").and_then(|i| i.as_i64()) {
                user_id_str = id.to_string();
                if let Some(expected_id) = expected_chat_id {
                    if id != expected_id {
                        warn!(
                            "Telegram user ID {} does not match authorized chat_id {}",
                            id, expected_id
                        );
                        return Err("Unauthorized Telegram User ID");
                    }
                }
            }
        }
    }

    Ok(AuthenticatedIdentity {
        auth_type: "telegram".to_string(),
        user_identifier: user_id_str,
    })
}

fn urlencoding_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            let hex_str: String = chars.by_ref().take(2).collect();
            if hex_str.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex_str, 16) {
                    result.push(byte as char);
                    continue;
                }
            }
            result.push('%');
            result.push_str(&hex_str);
        } else if ch == '+' {
            result.push(' ');
        } else {
            result.push(ch);
        }
    }
    result
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0;
    for (ca, cb) in a.bytes().zip(b.bytes()) {
        diff |= ca ^ cb;
    }
    diff == 0
}

/// Validates request authentication headers across multiple strategies
pub fn validate_auth(headers: &HeaderMap, query_str: Option<&str>, config: &Config) -> Result<AuthenticatedIdentity, (StatusCode, &'static str)> {
    // Strategy 1: Cloudflare Zero Trust Header
    if config.web.enable_cf_auth {
        if let Some(cf_email_val) = headers.get("cf-access-authenticated-user-email") {
            if let Ok(email) = cf_email_val.to_str() {
                if !email.is_empty() {
                    if config.web.cf_allowed_emails.is_empty()
                        || config.web.cf_allowed_emails.iter().any(|e| e.eq_ignore_ascii_case(email))
                    {
                        return Ok(AuthenticatedIdentity {
                            auth_type: "cloudflare_zero_trust".to_string(),
                            user_identifier: email.to_string(),
                        });
                    } else {
                        warn!("Cloudflare Zero Trust email '{}' not in allowed list", email);
                        return Err((StatusCode::FORBIDDEN, "Cloudflare Zero Trust email unauthorized"));
                    }
                }
            }
        }
    }

    // Strategy 2: Telegram Mini App `X-TG-Init-Data` or `X-Telegram-Init-Data` header
    if config.web.enable_tg_auth {
        if let Some(tg_header) = headers.get("x-tg-init-data").or_else(|| headers.get("x-telegram-init-data")) {
            if let Ok(init_data) = tg_header.to_str() {
                if let Some(bot_token) = &config.telegram.bot_token {
                    match verify_telegram_init_data(init_data, bot_token, config.telegram.chat_id) {
                        Ok(identity) => return Ok(identity),
                        Err(e) => {
                            warn!("Telegram WebApp initData verification failed: {}", e);
                            return Err((StatusCode::UNAUTHORIZED, "Invalid Telegram authentication"));
                        }
                    }
                }
            }
        }
    }

    // Strategy 3: Bearer Token / API Token Header / Query Param
    if let Some(ref required_token) = config.web.auth_token {
        if !required_token.is_empty() {
            // Check Authorization: Bearer <token>
            if let Some(auth_val) = headers.get("authorization") {
                if let Ok(auth_str) = auth_val.to_str() {
                    if let Some(token) = auth_str.strip_prefix("Bearer ").or_else(|| auth_str.strip_prefix("bearer ")) {
                        if constant_time_eq(token.trim(), required_token.trim()) {
                            return Ok(AuthenticatedIdentity {
                                auth_type: "bearer_token".to_string(),
                                user_identifier: "token_holder".to_string(),
                            });
                        }
                    }
                }
            }

            // Check X-Auth-Token
            if let Some(token_val) = headers.get("x-auth-token") {
                if let Ok(token) = token_val.to_str() {
                    if constant_time_eq(token.trim(), required_token.trim()) {
                        return Ok(AuthenticatedIdentity {
                            auth_type: "token".to_string(),
                            user_identifier: "token_holder".to_string(),
                        });
                    }
                }
            }

            // Check query parameter `?token=...`
            if let Some(q) = query_str {
                for part in q.split('&') {
                    if let Some((k, v)) = part.split_once('=') {
                        if k == "token" && constant_time_eq(v.trim(), required_token.trim()) {
                            return Ok(AuthenticatedIdentity {
                                auth_type: "query_token".to_string(),
                                user_identifier: "token_holder".to_string(),
                            });
                        }
                    }
                }
            }

            return Err((StatusCode::UNAUTHORIZED, "Missing or invalid authorization token"));
        }
    }

    // Strategy 4: Default Open / Loopback Access (if no token / credentials configured)
    if config.web.auth_token.is_none() && !config.web.enable_cf_auth && !config.web.enable_tg_auth {
        return Ok(AuthenticatedIdentity {
            auth_type: "anonymous_open".to_string(),
            user_identifier: "local".to_string(),
        });
    }

    // If any auth strategy was configured but none matched
    if config.web.auth_token.is_some() || !config.web.cf_allowed_emails.is_empty() {
        return Err((StatusCode::UNAUTHORIZED, "Authentication required"));
    }

    Ok(AuthenticatedIdentity {
        auth_type: "default_allowed".to_string(),
        user_identifier: "admin".to_string(),
    })
}

/// Axum middleware for API route protection
pub async fn auth_middleware(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<crate::web::state::AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, (StatusCode, &'static str)> {
    // Preflight OPTIONS requests don't require auth
    if req.method() == axum::http::Method::OPTIONS {
        return Ok(next.run(req).await);
    }

    // If requesting static HTML/CSS/JS frontend entry point, allow through
    let path = req.uri().path();
    if path == "/" || path == "/index.html" || path.starts_with("/static") || path == "/favicon.ico" {
        return Ok(next.run(req).await);
    }

    let cfg = state.config.load();
    let headers = req.headers();
    let query = req.uri().query();
    validate_auth(headers, query, &cfg)?;

    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telegram_init_data_verification() {
        let bot_token = "123456789:ABCdefGHIjklMNOpqrSTUvwxYZ";
        let mut mac_secret = HmacSha256::new_from_slice(b"WebAppData").unwrap();
        mac_secret.update(bot_token.as_bytes());
        let secret_key = mac_secret.finalize().into_bytes();

        let fresh_ts = chrono::Utc::now().timestamp();
        let fresh_query = format!("auth_date={}&query_id=AAH&user=%7B%22id%22%3A987654321%2C%22first_name%22%3A%22Alex%22%7D", fresh_ts);
        let fresh_check_str = format!("auth_date={}\nquery_id=AAH\nuser={{\"id\":987654321,\"first_name\":\"Alex\"}}", fresh_ts);
        
        let mut mac_fresh = HmacSha256::new_from_slice(&secret_key).unwrap();
        mac_fresh.update(fresh_check_str.as_bytes());
        let fresh_hash = hex::encode(mac_fresh.finalize().into_bytes());
        let fresh_init_data = format!("{}&hash={}", fresh_query, fresh_hash);

        let res_fresh = verify_telegram_init_data(&fresh_init_data, bot_token, Some(987654321));
        assert!(res_fresh.is_ok());
        let ident = res_fresh.unwrap();
        assert_eq!(ident.user_identifier, "987654321");
    }
}
