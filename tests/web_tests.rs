use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use bhyper::config::Config;
use bhyper::state::StateStore;
use bhyper::types::{Exchange, FundingRateInfo};
use bhyper::web::{auth::verify_telegram_init_data, build_router, AppState};
use bhyper::ws::MarketDataCache;
use chrono::Utc;
use hmac::{Hmac, Mac};
use http_body_util::BodyExt;
use parking_lot::Mutex;
use serde_json::Value;
use sha2::Sha256;
use std::sync::Arc;
use tempfile::NamedTempFile;
use tower::ServiceExt;

type HmacSha256 = Hmac<Sha256>;

fn create_test_state(config: Config) -> (Arc<AppState>, NamedTempFile) {
    let tmp_cfg_file = NamedTempFile::new().unwrap();
    let tmp_state_file = NamedTempFile::new().unwrap();

    let state_store = Arc::new(Mutex::new(
        StateStore::load_or_create(Some(tmp_state_file.path().to_path_buf())).unwrap(),
    ));

    let cache = MarketDataCache::new();
    // Feed mock Binance & Hyperliquid market rates
    cache.update_binance_rates(vec![
        FundingRateInfo {
            symbol: "BTC".into(),
            exchange: Exchange::Binance,
            mark_price: 65000.0,
            index_price: 65000.0,
            funding_rate: 0.0001, // 0.01%
            funding_interval_hours: 8.0,
            annualized_apr_pct: 10.95,
            next_funding_time: Some(Utc::now()),
        },
        FundingRateInfo {
            symbol: "SOL".into(),
            exchange: Exchange::Binance,
            mark_price: 150.0,
            index_price: 150.0,
            funding_rate: -0.0002, // -0.02%
            funding_interval_hours: 8.0,
            annualized_apr_pct: -21.9,
            next_funding_time: Some(Utc::now()),
        },
    ]);

    cache.update_hyperliquid_rates(vec![
        FundingRateInfo {
            symbol: "BTC".into(),
            exchange: Exchange::Hyperliquid,
            mark_price: 65010.0,
            index_price: 65005.0,
            funding_rate: 0.00005, // 0.005%/h
            funding_interval_hours: 1.0,
            annualized_apr_pct: 43.8,
            next_funding_time: Some(Utc::now()),
        },
        FundingRateInfo {
            symbol: "SOL".into(),
            exchange: Exchange::Hyperliquid,
            mark_price: 150.1,
            index_price: 150.05,
            funding_rate: 0.00003, // 0.003%/h
            funding_interval_hours: 1.0,
            annualized_apr_pct: 26.28,
            next_funding_time: Some(Utc::now()),
        },
    ]);

    let app_state = Arc::new(AppState::new(
        config,
        tmp_cfg_file.path().to_path_buf(),
        state_store,
        None,
        cache,
    ));

    (app_state, tmp_cfg_file)
}

#[tokio::test]
async fn test_web_embedded_static_index_html() {
    let config = Config::default();
    let (state, _tmp) = create_test_state(config);
    let app = build_router(state);

    let req = Request::builder()
        .uri("/")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8_lossy(&body_bytes);
    assert!(body_str.contains("BHyper"));
    assert!(body_str.contains("Telegram"));

    // Test CSS route
    let req_css = Request::builder()
        .uri("/css/theme.css")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let res_css = app.clone().oneshot(req_css).await.unwrap();
    assert_eq!(res_css.status(), StatusCode::OK);
    assert_eq!(res_css.headers().get("content-type").unwrap(), "text/css; charset=utf-8");

    // Test JS route
    let req_js = Request::builder()
        .uri("/js/app.js")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let res_js = app.oneshot(req_js).await.unwrap();
    assert_eq!(res_js.status(), StatusCode::OK);
    assert_eq!(res_js.headers().get("content-type").unwrap(), "application/javascript; charset=utf-8");
}

#[tokio::test]
async fn test_web_status_and_scan_endpoints() {
    let config = Config::default();
    let (state, _tmp) = create_test_state(config);
    let app = build_router(state);

    // 1. Test GET /api/status
    let req_status = Request::builder()
        .uri("/api/status")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let res_status = app.clone().oneshot(req_status).await.unwrap();
    assert_eq!(res_status.status(), StatusCode::OK);
    let body_bytes = res_status.into_body().collect().await.unwrap().to_bytes();
    let status_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(status_json["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(status_json["market_cache_symbols_count"], 2);

    // 2. Test GET /api/scan
    let req_scan = Request::builder()
        .uri("/api/scan")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let res_scan = app.oneshot(req_scan).await.unwrap();
    assert_eq!(res_scan.status(), StatusCode::OK);
    let scan_bytes = res_scan.into_body().collect().await.unwrap().to_bytes();
    let scan_json: Value = serde_json::from_slice(&scan_bytes).unwrap();
    assert_eq!(scan_json["count"], 2);
    let opps = scan_json["opportunities"].as_array().unwrap();
    assert!(!opps.is_empty());
}

#[tokio::test]
async fn test_web_config_get_and_hot_reload() {
    let mut config = Config::default();
    config.strategy.min_open_apr_pct = 25.0;
    config.binance.api_secret = "secret_binance_key_xyz".to_string();

    let (state, _tmp) = create_test_state(config.clone());
    let app = build_router(state.clone());

    // 1. Test GET /api/config (Secret masking)
    let req_get = Request::builder()
        .uri("/api/config")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let res_get = app.clone().oneshot(req_get).await.unwrap();
    assert_eq!(res_get.status(), StatusCode::OK);
    let bytes = res_get.into_body().collect().await.unwrap().to_bytes();
    let cfg_json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(cfg_json["binance"]["api_secret"], "********");
    assert_eq!(cfg_json["strategy"]["min_open_apr_pct"], 25.0);

    // 2. Test POST /api/config (Hot Update)
    let mut updated_config = config.clone();
    updated_config.strategy.min_open_apr_pct = 45.5;
    updated_config.strategy.max_position_usd_per_pair = 200.0;
    let payload = serde_json::to_string(&updated_config).unwrap();

    let req_post = Request::builder()
        .uri("/api/config")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload))
        .unwrap();

    let res_post = app.oneshot(req_post).await.unwrap();
    assert_eq!(res_post.status(), StatusCode::OK);

    // Verify in-memory state is atomically updated immediately
    let live_cfg = state.config.load();
    assert_eq!(live_cfg.strategy.min_open_apr_pct, 45.5);
    assert_eq!(live_cfg.strategy.max_position_usd_per_pair, 200.0);
    // Verify secret key preserved
    assert_eq!(live_cfg.binance.api_secret, "secret_binance_key_xyz");
}

#[tokio::test]
async fn test_telegram_mini_app_hmac_verification() {
    let bot_token = "987654321:AAFakeBotTokenForTestingOnly_123456";
    let chat_id: i64 = 1122334455;

    let now_ts = Utc::now().timestamp();
    let user_json = format!(
        "{{\"id\":{},\"first_name\":\"Trader\",\"username\":\"bhyper_quant\"}}",
        chat_id
    );
    let query_data = format!("auth_date={}&query_id=AAGH&user={}", now_ts, user_json);

    // 1. Calculate HMAC secret_key = HMAC_SHA256("WebAppData", bot_token)
    let mut mac_secret = HmacSha256::new_from_slice(b"WebAppData").unwrap();
    mac_secret.update(bot_token.as_bytes());
    let secret_key = mac_secret.finalize().into_bytes();

    // 2. Data check string
    let check_str = format!("auth_date={}\nquery_id=AAGH\nuser={}", now_ts, user_json);
    let mut mac_data = HmacSha256::new_from_slice(&secret_key).unwrap();
    mac_data.update(check_str.as_bytes());
    let valid_hash = hex::encode(mac_data.finalize().into_bytes());

    let full_init_data = format!("{}&hash={}", query_data, valid_hash);

    // Test successful verification
    let res = verify_telegram_init_data(&full_init_data, bot_token, Some(chat_id));
    assert!(res.is_ok());
    let identity = res.unwrap();
    assert_eq!(identity.user_identifier, chat_id.to_string());
    assert_eq!(identity.auth_type, "telegram");

    // Test tampered hash rejection
    let tampered_init_data = format!("{}&hash=badhash1234567890abcdef", query_data);
    let res_tampered = verify_telegram_init_data(&tampered_init_data, bot_token, Some(chat_id));
    assert!(res_tampered.is_err());

    // Test mismatched chat_id rejection
    let res_wrong_user = verify_telegram_init_data(&full_init_data, bot_token, Some(999999999));
    assert!(res_wrong_user.is_err());
}

#[tokio::test]
async fn test_auth_middleware_with_bearer_token() {
    let mut config = Config::default();
    config.web.auth_token = Some("secure_arbitrage_key_2026".to_string());

    let (state, _tmp) = create_test_state(config);
    let app = build_router(state);

    // 1. Request without auth -> Should be 401 Unauthorized
    let req_unauth = Request::builder()
        .uri("/api/status")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let res_unauth = app.clone().oneshot(req_unauth).await.unwrap();
    assert_eq!(res_unauth.status(), StatusCode::UNAUTHORIZED);

    // 2. Request with valid Bearer token -> Should succeed 200 OK
    let req_auth = Request::builder()
        .uri("/api/status")
        .method("GET")
        .header(header::AUTHORIZATION, "Bearer secure_arbitrage_key_2026")
        .body(Body::empty())
        .unwrap();

    let res_auth = app.clone().oneshot(req_auth).await.unwrap();
    assert_eq!(res_auth.status(), StatusCode::OK);

    // 3. Request with valid Query param `?token=...` -> Should succeed 200 OK
    let req_query = Request::builder()
        .uri("/api/status?token=secure_arbitrage_key_2026")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let res_query = app.oneshot(req_query).await.unwrap();
    assert_eq!(res_query.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_web_journal_health_and_paper_trade_actions() {
    let config = Config::default();
    let (state, _tmp) = create_test_state(config);
    let app = build_router(state);

    // 1. GET /api/health
    let req_health = Request::builder()
        .uri("/api/health")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let res_health = app.clone().oneshot(req_health).await.unwrap();
    assert_eq!(res_health.status(), StatusCode::OK);

    // 2. GET /api/positions
    let req_pos = Request::builder()
        .uri("/api/positions")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let res_pos = app.clone().oneshot(req_pos).await.unwrap();
    assert_eq!(res_pos.status(), StatusCode::OK);

    // 3. POST /api/action/paper_trade (Open BTC)
    let payload = serde_json::json!({
        "symbol": "BTC",
        "margin_usd": 50.0,
        "action": "open"
    });
    let req_open = Request::builder()
        .uri("/api/action/paper_trade")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();
    let res_open = app.clone().oneshot(req_open).await.unwrap();
    assert_eq!(res_open.status(), StatusCode::OK);
    let body_bytes = res_open.into_body().collect().await.unwrap().to_bytes();
    let open_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(open_json["status"], "ok");

    // 4. GET /api/journal (Filter by OPEN)
    let req_journal = Request::builder()
        .uri("/api/journal?event_type=OPEN&limit=10")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let res_journal = app.clone().oneshot(req_journal).await.unwrap();
    assert_eq!(res_journal.status(), StatusCode::OK);
    let j_bytes = res_journal.into_body().collect().await.unwrap().to_bytes();
    let j_json: Value = serde_json::from_slice(&j_bytes).unwrap();
    assert!(j_json["entries"].is_array());

    // 5. POST /api/action/unwind (Unwind ALL)
    let req_unwind = Request::builder()
        .uri("/api/action/unwind")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&serde_json::json!({ "symbol": "ALL" })).unwrap()))
        .unwrap();
    let res_unwind = app.oneshot(req_unwind).await.unwrap();
    assert_eq!(res_unwind.status(), StatusCode::OK);
}

