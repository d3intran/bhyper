use crate::hyperliquid::signing::{
    CancelWire, ExchangeAction, ExchangeRequestPayload, HyperliquidSigner, LimitWire,
    OrderTypeWire, OrderWire,
};
use crate::types::{Exchange, FundingRateInfo};
use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct HyperliquidClient {
    private_key: String,
    wallet_address: String,
    base_url: String,
    is_mainnet: bool,
    http_client: reqwest::Client,
}

#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)]
pub struct UniverseItem {
    pub name: String,
    #[serde(rename = "szDecimals")]
    pub sz_decimals: u32,
    #[serde(rename = "maxLeverage")]
    pub max_leverage: u32,
}

#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)]
pub struct AssetCtxItem {
    pub funding: String,
    #[serde(rename = "openInterest")]
    pub open_interest: String,
    #[serde(rename = "markPx")]
    pub mark_px: String,
    #[serde(rename = "oraclePx")]
    pub oracle_px: Option<String>,
    #[serde(rename = "midPx")]
    pub mid_px: Option<String>,
    #[serde(rename = "premium")]
    pub premium: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MetaPart {
    pub universe: Vec<UniverseItem>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ClearinghouseStateResponse {
    pub margin_summary: MarginSummary,
    pub cross_margin_summary: MarginSummary,
    pub asset_positions: Vec<AssetPositionWrapper>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct MarginSummary {
    pub account_value: String,
    pub total_margin_used: String,
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

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
pub struct OpenOrderItem {
    pub coin: String,
    pub limit_px: String,
    pub oid: u64,
    pub side: String,
    pub sz: String,
    pub timestamp: i64,
}

#[allow(dead_code)]
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

        let is_mainnet = !base_url.contains("testnet");

        Self {
            private_key,
            wallet_address,
            base_url,
            is_mainnet,
            http_client,
        }
    }

    fn timestamp_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// 获取所有交易对的 Meta 元信息 (universe 映射)
    pub async fn fetch_meta(&self) -> Result<MetaPart> {
        let url = format!("{}/info", self.base_url);
        let payload = json!({ "type": "meta" });

        let resp = self
            .http_client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("Failed to request Hyperliquid /info meta")?;

        let meta: MetaPart = resp
            .json()
            .await
            .context("Failed to parse Hyperliquid meta JSON")?;

        Ok(meta)
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

    /// 获取当前用户的挂单列表
    pub async fn fetch_open_orders(&self) -> Result<Vec<OpenOrderItem>> {
        if self.wallet_address.is_empty() {
            anyhow::bail!("Hyperliquid wallet address is not configured");
        }
        let url = format!("{}/info", self.base_url);
        let payload = json!({
            "type": "openOrders",
            "user": self.wallet_address
        });

        let resp = self
            .http_client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("Failed to request Hyperliquid openOrders")?;

        let orders: Vec<OpenOrderItem> = resp
            .json()
            .await
            .context("Failed to parse Hyperliquid openOrders JSON")?;

        Ok(orders)
    }

    /// 发送 L1 下单请求 (支持 Post-Only Maker, GTC Limit, IOC)
    pub async fn place_order(
        &self,
        asset_index: u32,
        is_buy: bool,
        price: f64,
        size: f64,
        reduce_only: bool,
        is_post_only: bool,
        is_ioc: bool,
    ) -> Result<serde_json::Value> {
        if self.private_key.is_empty() {
            anyhow::bail!("Hyperliquid private key is required for placing orders");
        }

        let tif = if is_post_only {
            "Alo" // Add Liquidity Only (Post-Only Maker)
        } else if is_ioc {
            "Ioc" // Immediate-Or-Cancel
        } else {
            "Gtc" // Good-Til-Cancelled
        };

        let price_str = format!("{:.6}", price)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string();
        let size_str = format!("{:.6}", size)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string();

        let action = ExchangeAction::Order {
            orders: vec![OrderWire {
                a: asset_index,
                b: is_buy,
                p: price_str,
                s: size_str,
                r: reduce_only,
                t: OrderTypeWire {
                    limit: LimitWire {
                        tif: tif.to_string(),
                    },
                },
            }],
            grouping: "na".to_string(),
        };

        let nonce = Self::timestamp_ms();
        let signature = HyperliquidSigner::sign_l1_action(
            &action,
            nonce,
            &self.private_key,
            self.is_mainnet,
        )?;

        let payload = ExchangeRequestPayload {
            action,
            nonce,
            signature,
            vault_address: None,
        };

        let url = format!("{}/exchange", self.base_url);
        let resp = self
            .http_client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("Failed to send Hyperliquid order request")?;

        let res_json: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse Hyperliquid order response")?;

        Ok(res_json)
    }

    /// 撤销 Hyperliquid 挂单
    pub async fn cancel_order(&self, asset_index: u32, oid: u64) -> Result<serde_json::Value> {
        if self.private_key.is_empty() {
            anyhow::bail!("Hyperliquid private key is required for cancelling orders");
        }

        let action = ExchangeAction::Cancel {
            cancels: vec![CancelWire {
                a: asset_index,
                o: oid,
            }],
        };

        let nonce = Self::timestamp_ms();
        let signature = HyperliquidSigner::sign_l1_action(
            &action,
            nonce,
            &self.private_key,
            self.is_mainnet,
        )?;

        let payload = ExchangeRequestPayload {
            action,
            nonce,
            signature,
            vault_address: None,
        };

        let url = format!("{}/exchange", self.base_url);
        let resp = self
            .http_client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("Failed to send Hyperliquid cancel request")?;

        let res_json: serde_json::Value = resp
            .json()
            .await
            .context("Failed to parse Hyperliquid cancel response")?;

        Ok(res_json)
    }
}
