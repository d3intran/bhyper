use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub binance: BinanceConfig,
    #[serde(default)]
    pub hyperliquid: HyperliquidConfig,
    #[serde(default)]
    pub strategy: StrategyConfig,
    #[serde(default)]
    pub risk: RiskConfig,
    #[serde(default)]
    pub telegram: TelegramConfig,
    #[serde(default)]
    pub web: WebConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinanceConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub api_secret: String,
    #[serde(default = "default_binance_base_url")]
    pub base_url: String,
}

fn default_binance_base_url() -> String {
    "https://fapi.binance.com".to_string()
}

impl Default for BinanceConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_secret: String::new(),
            base_url: default_binance_base_url(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperliquidConfig {
    #[serde(default)]
    pub private_key: String,
    #[serde(default)]
    pub wallet_address: String,
    #[serde(default = "default_hl_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub is_testnet: bool,
}

fn default_hl_base_url() -> String {
    "https://api.hyperliquid.xyz".to_string()
}

impl Default for HyperliquidConfig {
    fn default() -> Self {
        Self {
            private_key: String::new(),
            wallet_address: String::new(),
            base_url: default_hl_base_url(),
            is_testnet: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    #[serde(default = "default_symbols")]
    pub tracked_symbols: Vec<String>,
    #[serde(default = "default_min_apr")]
    pub min_open_apr_pct: f64,
    #[serde(default = "default_min_apr")]
    pub min_carry_apr_pct: f64,
    #[serde(default = "default_exit_apr")]
    pub min_exit_apr_pct: f64,
    #[serde(default = "default_max_pos_usd")]
    pub max_position_usd_per_pair: f64,
    #[serde(default = "default_max_active_positions")]
    pub max_active_positions: usize,
    #[serde(default = "default_max_holding_hours")]
    pub max_holding_hours: f64,
    #[serde(default = "default_stop_loss_basis_bps")]
    pub stop_loss_basis_bps: f64,
    #[serde(default = "default_take_profit_basis_bps")]
    pub take_profit_basis_bps: f64,
    #[serde(default = "default_leverage")]
    pub leverage: f64,
    #[serde(default = "default_true")]
    pub maker_taker_mode: bool,
    #[serde(default = "default_slippage_bps")]
    pub max_slippage_bps: f64,
    #[serde(default = "default_true")]
    pub auto_unwind_on_decay: bool,
    #[serde(default = "default_true")]
    pub fee_amortization_lock: bool,
    #[serde(default = "default_true")]
    pub dual_horizon_mode: bool,
    #[serde(default = "default_min_open_interest")]
    pub min_open_interest_usd: f64,
    #[serde(default = "default_min_volume_24h")]
    pub min_24h_volume_usd: f64,
    #[serde(default = "default_max_spread_bps")]
    pub max_bid_ask_spread_bps: f64,
    #[serde(default = "default_max_divergence_pct")]
    pub max_oracle_mark_divergence_pct: f64,
    #[serde(default)]
    pub symbol_whitelist: Vec<String>,
    #[serde(default)]
    pub symbol_blacklist: Vec<String>,
    #[serde(default = "default_true")]
    pub use_binance_ws_api: bool,
}

fn default_symbols() -> Vec<String> {
    vec![
        "BTC".into(),
        "ETH".into(),
        "SOL".into(),
        "SUI".into(),
        "DOGE".into(),
        "AVAX".into(),
        "LINK".into(),
        "NEAR".into(),
        "APT".into(),
        "ARB".into(),
        "OP".into(),
        "PEPE".into(),
        "WIF".into(),
        "TAO".into(),
        "RENDER".into(),
    ]
}

fn default_min_apr() -> f64 {
    25.0
}
fn default_exit_apr() -> f64 {
    5.0
}
fn default_max_pos_usd() -> f64 {
    120.0
}
fn default_max_active_positions() -> usize {
    3
}
fn default_max_holding_hours() -> f64 {
    12.0
}
fn default_stop_loss_basis_bps() -> f64 {
    40.0
}
fn default_take_profit_basis_bps() -> f64 {
    15.0
}
fn default_leverage() -> f64 {
    2.0
}
fn default_true() -> bool {
    true
}
fn default_slippage_bps() -> f64 {
    15.0
}
fn default_min_open_interest() -> f64 {
    500_000.0
}
fn default_min_volume_24h() -> f64 {
    1_000_000.0
}
fn default_max_spread_bps() -> f64 {
    15.0
}
fn default_max_divergence_pct() -> f64 {
    0.5
}

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            tracked_symbols: default_symbols(),
            min_open_apr_pct: default_min_apr(),
            min_carry_apr_pct: default_min_apr(),
            min_exit_apr_pct: default_exit_apr(),
            max_position_usd_per_pair: default_max_pos_usd(),
            max_active_positions: default_max_active_positions(),
            max_holding_hours: default_max_holding_hours(),
            stop_loss_basis_bps: default_stop_loss_basis_bps(),
            take_profit_basis_bps: default_take_profit_basis_bps(),
            leverage: default_leverage(),
            maker_taker_mode: default_true(),
            max_slippage_bps: default_slippage_bps(),
            auto_unwind_on_decay: default_true(),
            fee_amortization_lock: default_true(),
            dual_horizon_mode: default_true(),
            min_open_interest_usd: default_min_open_interest(),
            min_24h_volume_usd: default_min_volume_24h(),
            max_bid_ask_spread_bps: default_max_spread_bps(),
            max_oracle_mark_divergence_pct: default_max_divergence_pct(),
            symbol_whitelist: Vec::new(),
            symbol_blacklist: vec!["USTC".into(), "LUNC".into()],
            use_binance_ws_api: default_true(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    #[serde(default = "default_max_delta_pct")]
    pub max_delta_drift_pct: f64,
    #[serde(default = "default_min_margin_ratio")]
    pub min_margin_ratio_pct: f64,
    #[serde(default = "default_max_total_notional")]
    pub max_total_notional_usd: f64,
    #[serde(default = "default_true")]
    pub auto_rebalance_delta: bool,
    #[serde(default = "default_stop_loss_basis_bps")]
    pub stop_loss_basis_bps: f64,
    #[serde(default = "default_take_profit_basis_bps")]
    pub take_profit_basis_bps: f64,
    #[serde(default = "default_max_holding_hours")]
    pub max_holding_hours: f64,
    #[serde(default = "default_exit_apr")]
    pub min_exit_apr_pct: f64,
    #[serde(default = "default_true")]
    pub fee_amortization_lock: bool,
    #[serde(default = "default_max_margin_utilization")]
    pub max_margin_utilization_pct: f64,
    #[serde(default = "default_min_liquidation_distance")]
    pub min_liquidation_distance_pct: f64,
    #[serde(default = "default_rebalance_threshold")]
    pub rebalance_threshold_imbalance_pct: f64,
}

fn default_max_delta_pct() -> f64 {
    3.0
}
fn default_min_margin_ratio() -> f64 {
    25.0
}
fn default_max_total_notional() -> f64 {
    360.0
}
fn default_max_margin_utilization() -> f64 {
    75.0
}
fn default_min_liquidation_distance() -> f64 {
    20.0
}
fn default_rebalance_threshold() -> f64 {
    40.0
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_delta_drift_pct: default_max_delta_pct(),
            min_margin_ratio_pct: default_min_margin_ratio(),
            max_total_notional_usd: default_max_total_notional(),
            auto_rebalance_delta: default_true(),
            stop_loss_basis_bps: default_stop_loss_basis_bps(),
            take_profit_basis_bps: default_take_profit_basis_bps(),
            max_holding_hours: default_max_holding_hours(),
            min_exit_apr_pct: default_exit_apr(),
            fee_amortization_lock: default_true(),
            max_margin_utilization_pct: default_max_margin_utilization(),
            min_liquidation_distance_pct: default_min_liquidation_distance(),
            rebalance_threshold_imbalance_pct: default_rebalance_threshold(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TelegramConfig {
    pub bot_token: Option<String>,
    pub chat_id: Option<i64>,
    #[serde(default = "default_true")]
    pub alerts_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    #[serde(default = "default_web_host")]
    pub host: String,
    #[serde(default = "default_web_port")]
    pub port: u16,
    #[serde(default)]
    pub auth_token: Option<String>,
    #[serde(default = "default_true")]
    pub enable_tg_auth: bool,
    #[serde(default = "default_true")]
    pub enable_cf_auth: bool,
    #[serde(default)]
    pub cf_allowed_emails: Vec<String>,
}

fn default_web_host() -> String {
    "127.0.0.1".to_string()
}

fn default_web_port() -> u16 {
    8080
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            host: default_web_host(),
            port: default_web_port(),
            auth_token: None,
            enable_tg_auth: true,
            enable_cf_auth: true,
            cf_allowed_emails: Vec::new(),
        }
    }
}

impl Config {
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Result<Self> {
        let p = path.as_ref();
        if p.exists() {
            let content = std::fs::read_to_string(p)
                .with_context(|| format!("Failed to read config file: {}", p.display()))?;
            let mut cfg: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse TOML config: {}", p.display()))?;
            if cfg.binance.base_url.trim().is_empty() {
                cfg.binance.base_url = default_binance_base_url();
            }
            if cfg.hyperliquid.base_url.trim().is_empty() {
                cfg.hyperliquid.base_url = default_hl_base_url();
            }
            Ok(cfg)
        } else {
            let default_cfg = Config::default();
            if let Some(parent) = p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let toml_str = toml::to_string_pretty(&default_cfg)?;
            let _ = std::fs::write(p, toml_str);
            Ok(default_cfg)
        }
    }

    pub fn save_to<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let toml_str = toml::to_string_pretty(self)
            .context("Failed to serialize config to TOML")?;

        let tmp_path = p.with_extension("tmp");
        std::fs::write(&tmp_path, toml_str)
            .with_context(|| format!("Failed to write temporary config to {}", tmp_path.display()))?;
        std::fs::rename(&tmp_path, p)
            .with_context(|| format!("Failed to rename {} to {}", tmp_path.display(), p.display()))?;
        Ok(())
    }

    pub fn default_config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config/bhyper/config.toml")
    }
}
