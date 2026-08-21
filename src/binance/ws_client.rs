use anyhow::{bail, Result};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use rustc_hash::FxHashMap;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::connect_async;
use tracing::{error, info, warn};

#[allow(dead_code)]
pub struct BinanceWsApiClient {
    api_key: String,
    api_secret: String,
    hmac_key: Option<ring::hmac::Key>,
    ws_url: String,
    request_tx: mpsc::UnboundedSender<tokio_tungstenite::tungstenite::Message>,
    pending_requests: Arc<Mutex<FxHashMap<String, oneshot::Sender<Result<serde_json::Value>>>>>,
    req_counter: AtomicU64,
}

impl BinanceWsApiClient {
    pub fn spawn(api_key: String, api_secret: String, ws_url_opt: Option<String>) -> Arc<Self> {
        let ws_url = ws_url_opt.unwrap_or_else(|| "wss://ws-fapi.binance.com/ws-fapi/v1".to_string());
        let (request_tx, mut request_rx) = mpsc::unbounded_channel::<tokio_tungstenite::tungstenite::Message>();
        let pending_requests = Arc::new(Mutex::new(FxHashMap::default()));

        let hmac_key = if !api_secret.trim().is_empty() {
            Some(ring::hmac::Key::new(
                ring::hmac::HMAC_SHA256,
                api_secret.as_bytes(),
            ))
        } else {
            None
        };

        let client = Arc::new(Self {
            api_key,
            api_secret,
            hmac_key,
            ws_url: ws_url.clone(),
            request_tx,
            pending_requests: pending_requests.clone(),
            req_counter: AtomicU64::new(1),
        });

        let client_bg = client.clone();
        tokio::spawn(async move {
            loop {
                info!("Connecting to Binance WebSocket API at {}...", ws_url);
                match connect_async(&ws_url).await {
                    Ok((mut ws_stream, _)) => {
                        info!("Connected to Binance WebSocket API (ws-fapi)!");

                        let mut ping_timer = tokio::time::interval(Duration::from_secs(60));

                        loop {
                            tokio::select! {
                                _ = ping_timer.tick() => {
                                    if let Err(e) = ws_stream.send(tokio_tungstenite::tungstenite::Message::Ping(vec![])).await {
                                        warn!("Failed to send ping to Binance WS API: {:?}", e);
                                        break;
                                    }
                                }
                                outbound_msg = request_rx.recv() => {
                                    match outbound_msg {
                                        Some(msg) => {
                                            if let Err(e) = ws_stream.send(msg).await {
                                                error!("Failed to write message to Binance WS API: {:?}", e);
                                                break;
                                            }
                                        }
                                        None => return, // Client dropped
                                    }
                                }
                                inbound_msg = ws_stream.next() => {
                                    match inbound_msg {
                                        Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                                                if let Some(id) = val.get("id").and_then(|i| i.as_str()) {
                                                    let sender = {
                                                        let mut pending = client_bg.pending_requests.lock();
                                                        pending.remove(id)
                                                    };
                                                    if let Some(tx) = sender {
                                                        if let Some(err) = val.get("error") {
                                                            let _ = tx.send(Err(anyhow::anyhow!("Binance WS API error: {}", err)));
                                                        } else if let Some(result) = val.get("result") {
                                                            let _ = tx.send(Ok(result.clone()));
                                                        } else {
                                                            let _ = tx.send(Ok(val));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Some(Ok(tokio_tungstenite::tungstenite::Message::Ping(payload))) => {
                                            let _ = ws_stream.send(tokio_tungstenite::tungstenite::Message::Pong(payload)).await;
                                        }
                                        Some(Ok(_)) => {}
                                        Some(Err(e)) => {
                                            warn!("Binance WS API stream error: {:?}", e);
                                            break;
                                        }
                                        None => {
                                            warn!("Binance WS API stream closed by remote.");
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to connect to Binance WS API: {:?}. Retrying in 5s...", e);
                    }
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });

        client
    }

    fn timestamp_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Sign params and place an order using Binance WebSocket API
    pub async fn place_order(
        &self,
        symbol: &str,
        side: &str,
        order_type: &str,
        quantity: &str,
        price_opt: Option<&str>,
        reduce_only: bool,
    ) -> Result<serde_json::Value> {
        if self.api_key.is_empty() || self.hmac_key.is_none() {
            bail!("Binance API key or secret missing for WS API trade");
        }

        let pair = if symbol.ends_with("USDT") {
            symbol.to_string()
        } else {
            format!("{}USDT", symbol)
        };

        let req_id = format!("bhyper-ws-{}", self.req_counter.fetch_add(1, Ordering::Relaxed));
        let ts = Self::timestamp_ms();

        // Build canonical query payload for HMAC signature
        let mut query_params = vec![
            format!("apiKey={}", self.api_key),
            format!("quantity={}", quantity),
            format!("reduceOnly={}", if reduce_only { "true" } else { "false" }),
            format!("side={}", side),
            format!("symbol={}", pair),
            format!("timestamp={}", ts),
            format!("type={}", order_type),
        ];

        if let Some(px) = price_opt {
            query_params.push(format!("price={}", px));
            query_params.push("timeInForce=GTC".to_string());
        }

        query_params.sort();
        let payload_to_sign = query_params.join("&");

        let key = self.hmac_key.as_ref().unwrap();
        let signature_bytes = ring::hmac::sign(key, payload_to_sign.as_bytes());
        let signature = hex::encode(signature_bytes.as_ref());

        let mut params_map = serde_json::Map::new();
        params_map.insert("apiKey".to_string(), json!(self.api_key));
        params_map.insert("symbol".to_string(), json!(pair));
        params_map.insert("side".to_string(), json!(side));
        params_map.insert("type".to_string(), json!(order_type));
        params_map.insert("quantity".to_string(), json!(quantity));
        params_map.insert("reduceOnly".to_string(), json!(reduce_only.to_string()));
        params_map.insert("timestamp".to_string(), json!(ts));
        params_map.insert("signature".to_string(), json!(signature));

        if let Some(px) = price_opt {
            params_map.insert("price".to_string(), json!(px));
            params_map.insert("timeInForce".to_string(), json!("GTC"));
        }

        let ws_request = json!({
            "id": req_id,
            "method": "order.place",
            "params": params_map
        });

        let (resp_tx, resp_rx) = oneshot::channel();
        {
            let mut pending = self.pending_requests.lock();
            pending.insert(req_id.clone(), resp_tx);
        }

        let text_msg = tokio_tungstenite::tungstenite::Message::Text(ws_request.to_string());
        if let Err(e) = self.request_tx.send(text_msg) {
            let mut pending = self.pending_requests.lock();
            pending.remove(&req_id);
            bail!("Failed to dispatch WS order request to channel: {:?}", e);
        }

        // Wait with timeout
        match tokio::time::timeout(Duration::from_millis(3000), resp_rx).await {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => {
                bail!("Binance WS API response channel cancelled unexpectedly");
            }
            Err(_) => {
                let mut pending = self.pending_requests.lock();
                pending.remove(&req_id);
                bail!("Binance WS API order request timed out after 3000ms");
            }
        }
    }
}
