use crate::types::{Exchange, FundingRateInfo};
use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::json;

pub struct HyperliquidClient {
    #[allow(dead_code)]
    private_key: String,
    wallet_address: String,
    base_url: String,
    http_client: reqwest::Client,
}

#[derive(Deserialize, Debug)]
pub struct UniverseItem {
    pub name: String,
    #[allow(dead_code)]
    #[serde(rename = "szDecimals")]
    pub sz_decimals: u32,
    #[allow(dead_code)]
    #[serde(rename = "maxLeverage")]
    pub max_leverage: u32,
}

#[derive(Deserialize, Debug)]
pub struct AssetCtxItem {
    pub funding: String,
    #[allow(dead_code)]
    #[serde(rename = "openInterest")]
    pub open_interest: String,
    #[serde(rename = "markPx")]
    pub mark_px: String,
    #[serde(rename = "oraclePx")]
    pub oracle_px: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "midPx")]
    pub mid_px: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "premium")]
    pub premium: Option<String>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct MetaAndAssetCtxsResponse(pub MetaPart, pub Vec<AssetCtxItem>);

#[derive(Deserialize, Debug)]
pub struct MetaPart {
    pub universe: Vec<UniverseItem>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClearinghouseStateResponse {
    pub margin_summary: MarginSummary,
    #[allow(dead_code)]
    pub cross_margin_summary: MarginSummary,
    #[allow(dead_code)]
    pub asset_positions: Vec<AssetPositionWrapper>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MarginSummary {
    pub account_value: String,
    pub total_margin_used: String,
    #[allow(dead_code)]
    pub total_ntl_pos: String,
    pub total_raw_usd: String,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct AssetPositionWrapper {
    pub position: AssetPosition,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct AssetPosition {
    pub coin: String,
    pub szi: String,
    pub entry_px: Option<String>,
    pub position_value: String,
    pub unrealized_pnl: String,
    pub return_on_equity: String,
    pub liquidation_px: Option<String>,
    pub leverage: Option<serde_json::Value>,
}

impl HyperliquidClient {
    pub fn new(private_key: String, wallet_address: String, base_url: String) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let http_client = reqwest::Client::builder()
            .default_headers(headers)
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .tcp_nodelay(true)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let base_url = if base_url.trim().is_empty() {
            "https://api.hyperliquid.xyz".to_string()
        } else {
            base_url
        };
        Self {
            private_key,
            wallet_address,
            base_url,
            http_client,
        }
    }

    /// Fetches all active Hyperliquid universe and funding rate contexts
    pub async fn fetch_all_funding_rates(&self) -> Result<Vec<FundingRateInfo>> {
        let url = format!("{}/info", self.base_url);
        let payload = json!({ "type": "metaAndAssetCtxs" });

        let resp = self
            .http_client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("Failed to send request to Hyperliquid /info")?;

        let (meta, ctxs): (MetaPart, Vec<AssetCtxItem>) = resp
            .json()
            .await
            .context("Failed to parse Hyperliquid metaAndAssetCtxs JSON")?;

        let mut results = Vec::with_capacity(meta.universe.len());

        for (u, ctx) in meta.universe.iter().zip(ctxs.iter()) {
            let symbol = u.name.clone();
            let mark_p = ctx.mark_px.parse::<f64>().unwrap_or(0.0);
            let oracle_p = ctx
                .oracle_px
                .as_ref()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(mark_p);
            let rate_1h = ctx.funding.parse::<f64>().unwrap_or(0.0);
            // Hyperliquid: 1h rate. 24 settlements/day * 365 days = 8760 times per year.
            let apr = rate_1h * 8760.0 * 100.0;

            results.push(FundingRateInfo {
                symbol,
                exchange: Exchange::Hyperliquid,
                mark_price: mark_p,
                index_price: oracle_p,
                funding_rate: rate_1h,
                funding_interval_hours: 1.0,
                annualized_apr_pct: apr,
                next_funding_time: Some(Utc::now()),
            });
        }

        Ok(results)
    }

    /// Fetches the user clearinghouse state (margin, balances, positions)
    pub async fn fetch_clearinghouse_state(&self) -> Result<ClearinghouseStateResponse> {
        if self.wallet_address.is_empty() {
            anyhow::bail!("Hyperliquid wallet address is not configured");
        }
        let url = format!("{}/info", self.base_url);
        let payload = json!({
            "type": "clearinghouseState",
            "user": self.wallet_address
        });

        let resp = self
            .http_client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("Failed to request Hyperliquid clearinghouseState")?;

        let state: ClearinghouseStateResponse = resp
            .json()
            .await
            .context("Failed to parse Hyperliquid clearinghouseState JSON")?;

        Ok(state)
    }
}
