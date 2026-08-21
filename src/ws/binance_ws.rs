use crate::types::{Exchange, FundingRateInfo};
use crate::ws::market_cache::MarketDataCache;
use chrono::{TimeZone, Utc};
use futures_util::StreamExt;
use serde::Deserialize;
use std::time::Duration;
use tokio_tungstenite::connect_async;
use tracing::{error, info, warn};

#[derive(Deserialize, Debug)]
struct BinanceWsMarkPriceItem<'a> {
    #[serde(rename = "s", borrow)]
    symbol: &'a str,
    #[serde(rename = "p", borrow)]
    mark_price: &'a str,
    #[serde(rename = "i", borrow)]
    index_price: &'a str,
    #[serde(rename = "r", borrow)]
    funding_rate: &'a str,
    #[serde(rename = "T")]
    next_funding_time: i64,
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
                                    if let Ok(items) =
                                        serde_json::from_str::<Vec<BinanceWsMarkPriceItem>>(&text)
                                    {
                                        let mut rates = Vec::with_capacity(items.len());
                                        for item in items {
                                            if !item.symbol.ends_with("USDT") {
                                                continue;
                                            }
                                            let base_coin =
                                                item.symbol.trim_end_matches("USDT").to_string();
                                            let mark_p =
                                                item.mark_price.parse::<f64>().unwrap_or(0.0);
                                            let index_p =
                                                item.index_price.parse::<f64>().unwrap_or(0.0);
                                            let rate_8h =
                                                item.funding_rate.parse::<f64>().unwrap_or(0.0);
                                            let apr = rate_8h * 1095.0 * 100.0;
                                            let next_t = Utc
                                                .timestamp_millis_opt(item.next_funding_time)
                                                .single();

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
