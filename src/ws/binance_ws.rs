use crate::types::{Exchange, FundingRateInfo};
use crate::ws::market_cache::MarketDataCache;
use chrono::{TimeZone, Utc};
use futures_util::StreamExt;
use serde::Deserialize;
use std::time::Duration;
use tokio_tungstenite::connect_async;
use tracing::{error, info, warn};

#[derive(Deserialize, Debug)]
struct BinanceWsMarkPriceItem {
    #[serde(rename = "s", default)]
    symbol: Option<String>,
    #[serde(rename = "p", default)]
    mark_price: Option<String>,
    #[serde(rename = "i", default)]
    index_price: Option<String>,
    #[serde(rename = "r", default)]
    funding_rate: Option<String>,
    #[serde(rename = "T", default)]
    next_funding_time: Option<i64>,
}

pub struct BinanceWsStream;

impl BinanceWsStream {
    pub fn spawn(cache: MarketDataCache) {
        tokio::spawn(async move {
            let ws_url = "wss://fstream.binance.com/ws/!markPrice@arr@1s";
            loop {
                info!(
                    "Connecting to Binance FAPI WebSocket stream at {}...",
                    ws_url
                );
                match connect_async(ws_url).await {
                    Ok((mut ws_stream, _)) => {
                        info!("Connected to Binance FAPI WebSocket stream!");
                        while let Some(msg_res) = ws_stream.next().await {
                            match msg_res {
                                Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                                    let items_opt = if let Ok(items) =
                                        serde_json::from_str::<Vec<BinanceWsMarkPriceItem>>(&text)
                                    {
                                        Some(items)
                                    } else if let Ok(val) =
                                        serde_json::from_str::<serde_json::Value>(&text)
                                    {
                                        if let Some(arr) =
                                            val.get("data").and_then(|d| d.as_array())
                                        {
                                            serde_json::from_value::<Vec<BinanceWsMarkPriceItem>>(
                                                serde_json::Value::Array(arr.clone()),
                                            )
                                            .ok()
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    };

                                    if let Some(items) = items_opt {
                                        let mut rates = Vec::with_capacity(items.len());
                                        for item in items {
                                            let full_sym = match item.symbol {
                                                Some(s) => s,
                                                None => continue,
                                            };
                                            let base_coin = match full_sym.strip_suffix("USDT") {
                                                Some(b) => b.to_ascii_uppercase(),
                                                None => continue,
                                            };

                                            let mark_p = item
                                                .mark_price
                                                .and_then(|p| p.parse::<f64>().ok())
                                                .unwrap_or(0.0);
                                            let index_p = item
                                                .index_price
                                                .and_then(|p| p.parse::<f64>().ok())
                                                .unwrap_or(mark_p);
                                            let rate_8h = item
                                                .funding_rate
                                                .and_then(|r| r.parse::<f64>().ok())
                                                .unwrap_or(0.0);
                                            let apr = rate_8h * 1095.0 * 100.0;
                                            let next_t = item
                                                .next_funding_time
                                                .and_then(|t| Utc.timestamp_millis_opt(t).single());

                                            rates.push(FundingRateInfo {
                                                symbol: base_coin,
                                                exchange: Exchange::Binance,
                                                mark_price: mark_p,
                                                index_price: index_p,
                                                funding_rate: rate_8h,
                                                funding_interval_hours: 8.0,
                                                annualized_apr_pct: apr,
                                                next_funding_time: next_t,
                                            });
                                        }
                                        cache.update_binance_rates(rates);
                                    }
                                }
                                Ok(tokio_tungstenite::tungstenite::Message::Ping(p)) => {
                                    // Tungstenite automatically replies with pong
                                    let _ = p;
                                }
                                Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => {
                                    warn!("Binance WebSocket stream closed by remote server.");
                                    break;
                                }
                                Err(e) => {
                                    error!("Error reading Binance WebSocket message: {:?}", e);
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to connect to Binance WebSocket: {:?}. Retrying in 3s...",
                            e
                        );
                    }
                }
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        });
    }
}
