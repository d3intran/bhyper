use crate::types::{Exchange, FundingRateInfo};
use crate::ws::market_cache::{MarketDataCache, UserFillEvent};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio_tungstenite::connect_async;
use tracing::{error, info, warn};

#[derive(Deserialize, Debug)]
#[serde(tag = "channel")]
enum HlWsEnvelope<'a> {
    #[serde(rename = "allMids")]
    AllMids {
        #[serde(borrow)]
        data: HlAllMidsData<'a>,
    },
    #[serde(rename = "webData2")]
    WebData2 { data: serde_json::Value },
    #[serde(rename = "userFills")]
    UserFills {
        #[serde(borrow)]
        data: HlUserFillsData<'a>,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize, Debug)]
struct HlAllMidsData<'a> {
    #[serde(borrow)]
    mids: std::collections::HashMap<&'a str, &'a str>,
}

#[derive(Deserialize, Debug)]
struct HlUserFillsData<'a> {
    #[serde(borrow)]
    fills: Vec<HlFillWire<'a>>,
}

#[derive(Deserialize, Debug)]
struct HlFillWire<'a> {
    #[serde(borrow)]
    coin: &'a str,
    #[serde(borrow)]
    px: &'a str,
    #[serde(borrow)]
    sz: &'a str,
    #[serde(borrow)]
    side: &'a str,
    time: i64,
    #[serde(borrow)]
    fee: &'a str,
    oid: u64,
    tid: u64,
}

pub struct HyperliquidWsStream;

impl HyperliquidWsStream {
    pub fn spawn(
        cache: MarketDataCache,
        base_ws_url: Option<String>,
        wallet_address: Option<String>,
    ) {
        let ws_url = base_ws_url.unwrap_or_else(|| "wss://api.hyperliquid.xyz/ws".to_string());
        tokio::spawn(async move {
            loop {
                info!(
                    "Connecting to Hyperliquid L1 WebSocket stream at {}...",
                    ws_url
                );
                match connect_async(&ws_url).await {
                    Ok((mut ws_stream, _)) => {
                        info!("Connected to Hyperliquid WebSocket stream!");

                        // Subscribe to allMids
                        let all_mids_sub = json!({
                            "method": "subscribe",
                            "subscription": {
                                "type": "allMids"
                            }
                        });
                        let _ = ws_stream
                            .send(tokio_tungstenite::tungstenite::Message::Text(
                                all_mids_sub.to_string(),
                            ))
                            .await;

                        // If user address is configured, subscribe to webData2 and userFills
                        if let Some(ref addr) = wallet_address {
                            if !addr.is_empty() {
                                let web_data_sub = json!({
                                    "method": "subscribe",
                                    "subscription": {
                                        "type": "webData2",
                                        "user": addr
                                    }
                                });
                                let _ = ws_stream
                                    .send(tokio_tungstenite::tungstenite::Message::Text(
                                        web_data_sub.to_string(),
                                    ))
                                    .await;

                                let user_fills_sub = json!({
                                    "method": "subscribe",
                                    "subscription": {
                                        "type": "userFills",
                                        "user": addr
                                    }
                                });
                                let _ = ws_stream
                                    .send(tokio_tungstenite::tungstenite::Message::Text(
                                        user_fills_sub.to_string(),
                                    ))
                                    .await;
                                info!(
                                    "Subscribed to Hyperliquid webData2 and userFills for {}",
                                    addr
                                );
                            }
                        }

                        // Also spawn periodic ping
                        let mut ping_interval = tokio::time::interval(Duration::from_secs(30));

                        loop {
                            tokio::select! {
                                _ = ping_interval.tick() => {
                                    let ping_msg = json!({ "method": "ping" });
                                    if let Err(e) = ws_stream.send(tokio_tungstenite::tungstenite::Message::Text(ping_msg.to_string())).await {
                                        warn!("Failed to send ping to Hyperliquid WS: {:?}", e);
                                        break;
                                    }
                                }
                                msg_opt = ws_stream.next() => {
                                    match msg_opt {
                                        Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                                            if let Ok(envelope) = serde_json::from_str::<HlWsEnvelope>(&text) {
                                                match envelope {
                                                    HlWsEnvelope::AllMids { data } => {
                                                        let mut mids_map = std::collections::HashMap::with_capacity(data.mids.len());
                                                        for (sym, price_str) in data.mids {
                                                            if let Ok(price) = price_str.parse::<f64>() {
                                                                if price > 0.0 {
                                                                    mids_map.insert(sym.to_string(), price);
                                                                }
                                                            }
                                                        }
                                                        cache.update_hyperliquid_mids(mids_map);
                                                    }
                                                    HlWsEnvelope::UserFills { data } => {
                                                        for f in data.fills {
                                                            let px = f.px.parse::<f64>().unwrap_or(0.0);
                                                            let sz = f.sz.parse::<f64>().unwrap_or(0.0);
                                                            let fee = f.fee.parse::<f64>().unwrap_or(0.0);
                                                            info!("⚡ WS User Fill Received: {} {} {} @ ${:.4} (OID: {})", f.side, sz, f.coin, px, f.oid);
                                                            cache.record_user_fill(UserFillEvent {
                                                                coin: f.coin.to_string(),
                                                                px,
                                                                sz,
                                                                side: f.side.to_string(),
                                                                time: f.time,
                                                                fee,
                                                                oid: f.oid,
                                                                tid: f.tid,
                                                            });
                                                        }
                                                    }
                                                    HlWsEnvelope::WebData2 { data } => {
                                                        if let Some(mac) = data.get("metaAndAssetCtxs").and_then(|m| m.as_array()) {
                                                            if mac.len() >= 2 {
                                                                if let (Some(universe_val), Some(ctxs_val)) = (
                                                                    mac[0].get("universe").and_then(|u| u.as_array()),
                                                                    mac[1].as_array(),
                                                                ) {
                                                                    let mut rates = Vec::with_capacity(universe_val.len());
                                                                    for (u_val, ctx_val) in universe_val.iter().zip(ctxs_val.iter()) {
                                                                        let sym = u_val.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                                                        if sym.is_empty() {
                                                                            continue;
                                                                        }
                                                                        let mark_p = ctx_val.get("markPx").and_then(|m| m.as_str()).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                                                                        let oracle_p = ctx_val.get("oraclePx").and_then(|m| m.as_str()).and_then(|s| s.parse::<f64>().ok()).unwrap_or(mark_p);
                                                                        let rate_1h = ctx_val.get("funding").and_then(|f| f.as_str()).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                                                                        let apr = rate_1h * 8760.0 * 100.0;

                                                                        rates.push(FundingRateInfo {
                                                                            symbol: sym.to_ascii_uppercase(),
                                                                            exchange: Exchange::Hyperliquid,
                                                                            mark_price: mark_p,
                                                                            index_price: oracle_p,
                                                                            funding_rate: rate_1h,
                                                                            funding_interval_hours: 1.0,
                                                                            annualized_apr_pct: apr,
                                                                            next_funding_time: Some(Utc::now()),
                                                                        });
                                                                    }
                                                                    cache.update_hyperliquid_rates(rates);
                                                                }
                                                            }
                                                        }
                                                    }
                                                    HlWsEnvelope::Other => {}
                                                }
                                            }
                                        }
                                        Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) => {
                                            warn!("Hyperliquid WS closed by server.");
                                            break;
                                        }
                                        Some(Err(e)) => {
                                            error!("Hyperliquid WS error: {:?}", e);
                                            break;
                                        }
                                        None => {
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to connect to Hyperliquid WS: {:?}. Retrying in 3s...",
                            e
                        );
                    }
                }
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        });
    }
}
