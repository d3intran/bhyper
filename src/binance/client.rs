use crate::types::{Exchange, FundingRateInfo, PositionSide};
use anyhow::{Context, Result};
use chrono::{TimeZone, Utc};
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

pub struct BinanceFuturesClient {
    #[allow(dead_code)]
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
pub struct BinancePositionItem {
    pub symbol: String,
    pub position_amt: String,
    pub entry_price: String,
    pub mark_price: String,
    pub un_realized_profit: String,
    pub liquidation_price: String,
    pub leverage: String,
}

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

    /// Places an order on Binance FAPI (Taker Market or Maker Limit)
    #[allow(dead_code)]
    pub async fn place_order(
        &self,
        symbol: &str,
        side: PositionSide,
        qty: f64,
        price: Option<f64>,
        reduce_only: bool,
    ) -> Result<serde_json::Value> {
        let pair = format!("{}USDT", symbol);
        let side_str = match side {
            PositionSide::Long => "BUY",
            PositionSide::Short => "SELL",
        };
        let ts = Self::timestamp_ms();

        let query = if let Some(p) = price {
            format!(
                "symbol={}&side={}&type=LIMIT&timeInForce=GTC&quantity={}&price={}&reduceOnly={}&timestamp={}",
                pair, side_str, qty, p, reduce_only, ts
            )
        } else {
            format!(
                "symbol={}&side={}&type=MARKET&quantity={}&reduceOnly={}&timestamp={}",
                pair, side_str, qty, reduce_only, ts
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
}
