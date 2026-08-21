use crate::types::{Exchange, FundingRateInfo, PositionSide, SymbolPrecisionInfo};
use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use sha2::Sha256;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

#[allow(dead_code)]
pub struct BinanceFuturesClient {
    api_key: String,
    api_secret: String,
    base_url: String,
    http_client: reqwest::Client,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct PremiumIndexItem {
    symbol: String,
    mark_price: String,
    index_price: String,
    last_funding_rate: String,
    next_funding_time: i64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BinanceBalanceItem {
    pub asset: String,
    pub balance: String,
    pub available_balance: String,
    #[allow(dead_code)]
    pub cross_un_pnl: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct BinancePositionRiskItem {
    pub symbol: String,
    pub position_amt: String,
    pub entry_price: String,
    pub mark_price: String,
    pub un_realized_profit: String,
    pub liquidation_price: String,
    pub leverage: String,
    pub margin_type: String,
}

#[derive(Deserialize, Debug)]
struct ExchangeInfoResponse {
    symbols: Vec<ExchangeInfoSymbol>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ExchangeInfoSymbol {
    symbol: String,
    status: String,
    filters: Vec<serde_json::Value>,
}

#[allow(dead_code)]
impl BinanceFuturesClient {
    pub fn new(api_key: String, api_secret: String, base_url: String) -> Self {
        let mut headers = HeaderMap::new();
        if !api_key.is_empty() {
            if let Ok(val) = HeaderValue::from_str(&api_key) {
                headers.insert("X-MBX-APIKEY", val);
            }
        }
        let http_client = reqwest::Client::builder()
            .default_headers(headers)
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .tcp_nodelay(true)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let base_url = if base_url.trim().is_empty() {
            "https://fapi.binance.com".to_string()
        } else {
            base_url
        };
        Self {
            api_key,
            api_secret,
            base_url,
            http_client,
        }
    }

    fn sign_query(&self, query: &str) -> String {
        if self.api_secret.is_empty() {
            return query.to_string();
        }
        let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(query.as_bytes());
        let result = mac.finalize();
        let sig = hex::encode(result.into_bytes());
        format!("{}&signature={}", query, sig)
    }

    fn timestamp_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Fetches Binance exchangeInfo and parses precision filters for all symbols
    pub async fn fetch_precision_info(&self) -> Result<HashMap<String, SymbolPrecisionInfo>> {
        let url = format!("{}/fapi/v1/exchangeInfo", self.base_url);
        let resp = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to request Binance exchangeInfo")?;

        let info: ExchangeInfoResponse = resp
            .json()
            .await
            .context("Failed to parse Binance exchangeInfo JSON")?;

        let mut precisions = HashMap::with_capacity(info.symbols.len());

        for s in info.symbols {
            if s.status != "TRADING" || !s.symbol.ends_with("USDT") {
                continue;
            }
            let base_coin = s.symbol.trim_end_matches("USDT").to_string();

            let mut step_size = 1.0;
            let mut tick_size = 0.0001;
            let mut min_qty = 0.001;
            let mut min_notional = 5.0;

            for f in s.filters {
                let filter_type = f.get("filterType").and_then(|v| v.as_str()).unwrap_or("");
                match filter_type {
                    "LOT_SIZE" => {
                        if let Some(ss) = f.get("stepSize").and_then(|v| v.as_str()) {
                            step_size = ss.parse::<f64>().unwrap_or(1.0);
                        }
                        if let Some(mq) = f.get("minQty").and_then(|v| v.as_str()) {
                            min_qty = mq.parse::<f64>().unwrap_or(0.001);
                        }
                    }
                    "PRICE_FILTER" => {
                        if let Some(ts) = f.get("tickSize").and_then(|v| v.as_str()) {
                            tick_size = ts.parse::<f64>().unwrap_or(0.0001);
                        }
                    }
                    "MIN_NOTIONAL" => {
                        if let Some(mn) = f.get("notional").and_then(|v| v.as_str()) {
                            min_notional = mn.parse::<f64>().unwrap_or(5.0);
                        }
                    }
                    _ => {}
                }
            }

            precisions.insert(
                base_coin.clone(),
                SymbolPrecisionInfo {
                    symbol: base_coin,
                    binance_step_size: step_size,
                    binance_tick_size: tick_size,
                    binance_min_qty: min_qty,
                    binance_min_notional: min_notional,
                    hyperliquid_sz_decimals: 0,
                    hyperliquid_asset_index: 0,
                    hyperliquid_min_notional: 10.0,
                },
            );
        }

        Ok(precisions)
    }

    /// Fetches all active Premium Index & Funding Rate records from Binance FAPI
    pub async fn fetch_all_funding_rates(&self) -> Result<Vec<FundingRateInfo>> {
        let url = format!("{}/fapi/v1/premiumIndex", self.base_url);
        let resp = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to request Binance premiumIndex")?;

        let items: Vec<PremiumIndexItem> = resp
            .json()
            .await
            .context("Failed to parse Binance premiumIndex JSON")?;

        let mut results = Vec::with_capacity(items.len());
        for item in items {
            if !item.symbol.ends_with("USDT") {
                continue;
            }
            let base_coin = item.symbol.trim_end_matches("USDT").to_string();
            let mark_p = item.mark_price.parse::<f64>().unwrap_or(0.0);
            let index_p = item.index_price.parse::<f64>().unwrap_or(0.0);
            let rate_8h = item.last_funding_rate.parse::<f64>().unwrap_or(0.0);
            // Binance: 8h rate. 3 settlements/day * 365 days = 1095 times per year.
            let apr = rate_8h * 1095.0 * 100.0;

            let next_t = Utc.timestamp_millis_opt(item.next_funding_time).single();

            results.push(FundingRateInfo {
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

        Ok(results)
    }

    /// Fetches account USDT balance & margin info
    pub async fn fetch_balances(&self) -> Result<Vec<BinanceBalanceItem>> {
        let query = format!("timestamp={}", Self::timestamp_ms());
        let signed_query = self.sign_query(&query);
        let url = format!("{}/fapi/v2/balance?{}", self.base_url, signed_query);

        let resp = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to request Binance balance")?;

        let items: Vec<BinanceBalanceItem> = resp
            .json()
            .await
            .context("Failed to parse Binance balance JSON")?;

        Ok(items)
    }

    /// Fetches active position risks across all contracts
    pub async fn fetch_positions(&self) -> Result<Vec<BinancePositionRiskItem>> {
        let query = format!("timestamp={}", Self::timestamp_ms());
        let signed_query = self.sign_query(&query);
        let url = format!("{}/fapi/v2/positionRisk?{}", self.base_url, signed_query);

        let resp = self
            .http_client
            .get(&url)
            .send()
            .await
            .context("Failed to request Binance positionRisk")?;

        let items: Vec<BinancePositionRiskItem> = resp
            .json()
            .await
            .context("Failed to parse Binance positionRisk JSON")?;

        Ok(items)
    }

    /// Places an order on Binance FAPI (Taker Market or Maker Limit)
    pub async fn place_order(
        &self,
        symbol: &str,
        side: PositionSide,
        qty_str: &str,
        price_str: Option<&str>,
        reduce_only: bool,
    ) -> Result<serde_json::Value> {
        let pair = format!("{}USDT", symbol);
        let side_str = match side {
            PositionSide::Long => "BUY",
            PositionSide::Short => "SELL",
        };
        let ts = Self::timestamp_ms();

        let query = if let Some(p) = price_str {
            format!(
                "symbol={}&side={}&type=LIMIT&timeInForce=GTC&quantity={}&price={}&reduceOnly={}&timestamp={}",
                pair, side_str, qty_str, p, reduce_only, ts
            )
        } else {
            format!(
                "symbol={}&side={}&type=MARKET&quantity={}&reduceOnly={}&timestamp={}",
                pair, side_str, qty_str, reduce_only, ts
            )
        };

        let signed_query = self.sign_query(&query);
        let url = format!("{}/fapi/v1/order?{}", self.base_url, signed_query);

        let resp = self
            .http_client
            .post(&url)
            .send()
            .await
            .context("Failed to send Binance order")?;

        let json_val: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse Binance order response JSON")?;

        Ok(json_val)
    }

    /// Cancels an open order on Binance FAPI
    pub async fn cancel_order(&self, symbol: &str, order_id: u64) -> Result<serde_json::Value> {
        let pair = format!("{}USDT", symbol);
        let ts = Self::timestamp_ms();
        let query = format!("symbol={}&orderId={}&timestamp={}", pair, order_id, ts);
        let signed_query = self.sign_query(&query);
        let url = format!("{}/fapi/v1/order?{}", self.base_url, signed_query);

        let resp = self
            .http_client
            .delete(&url)
            .send()
            .await
            .context("Failed to cancel Binance order")?;

        let json_val: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse Binance cancel response JSON")?;

        Ok(json_val)
    }
}
