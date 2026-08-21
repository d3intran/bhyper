use crate::types::{Exchange, FundingRateInfo};
use crate::ws::market_cache::{MarketDataCache, UserFillEvent};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::time::Duration;
use tokio_tungstenite::connect_async;
use tracing::{error, info, warn};

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
                info!("Connecting to Hyperliquid L1 WebSocket stream at {}...", ws_url);
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

                        // If user address is configured, subscribe to userFills
                        if let Some(ref addr) = wallet_address {
                            if !addr.is_empty() {
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
                                info!("Subscribed to Hyperliquid userFills for {}", addr);
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
                                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                                                let channel = v.get("channel").and_then(|c| c.as_str()).unwrap_or("");
                                                if channel == "allMids" {
                                                    if let Some(data) = v.get("data").and_then(|d| d.get("mids")) {
                                                        if let Some(obj) = data.as_object() {
                                                            let mut rates = Vec::with_capacity(obj.len());
                                                            for (sym, price_val) in obj {
                                                                let price = price_val.as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                                                                if price > 0.0 {
                                                                    rates.push(FundingRateInfo {
                                                                        symbol: sym.to_uppercase(),
                                                                        exchange: Exchange::Hyperliquid,
                                                                        mark_price: price,
                                                                        index_price: price,
                                                                        funding_rate: 0.0, // Updated in tandem with REST context or activeAssetCtx
                                                                        funding_interval_hours: 1.0,
                                                                        annualized_apr_pct: 0.0,
                                                                        next_funding_time: Some(Utc::now()),
                                                                    });
                                                                }
                                                            }
                                                            cache.update_hyperliquid_rates(rates);
                                                        }
                                                    }
                                                } else if channel == "userFills" {
                                                    if let Some(fills_arr) = v.get("data").and_then(|d| d.get("fills")).and_then(|f| f.as_array()) {
                                                        for f in fills_arr {
                                                            let coin = f.get("coin").and_then(|c| c.as_str()).unwrap_or("").to_string();
                                                            let px = f.get("px").and_then(|p| p.as_str()).and_then(|p| p.parse::<f64>().ok()).unwrap_or(0.0);
                                                            let sz = f.get("sz").and_then(|s| s.as_str()).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                                                            let side = f.get("side").and_then(|s| s.as_str()).unwrap_or("").to_string();
                                                            let time = f.get("time").and_then(|t| t.as_i64()).unwrap_or(0);
                                                            let fee = f.get("fee").and_then(|fe| fe.as_str()).and_then(|fe| fe.parse::<f64>().ok()).unwrap_or(0.0);
                                                            let oid = f.get("oid").and_then(|o| o.as_u64()).unwrap_or(0);
                                                            let tid = f.get("tid").and_then(|t| t.as_u64()).unwrap_or(0);

                                                            info!("⚡ WS User Fill Received: {} {} {} @ ${:.4}", side, sz, coin, px);
                                                            cache.record_user_fill(UserFillEvent {
                                                                coin, px, sz, side, time, fee, oid, tid,
                                                            });
                                                        }
                                                    }
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
                        error!("Failed to connect to Hyperliquid WS: {:?}. Retrying in 3s...", e);
                    }
                }
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        });
    }
}
