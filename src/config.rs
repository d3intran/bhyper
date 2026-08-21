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
    30.0
}
fn default_exit_apr() -> f64 {
    5.0
}
fn default_max_pos_usd() -> f64 {
    500.0
}
fn default_max_active_positions() -> usize {
    3
}
fn default_max_holding_hours() -> f64 {
    8.0
}
fn default_stop_loss_basis_bps() -> f64 {
    40.0
}
fn default_take_profit_basis_bps() -> f64 {
    20.0
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

impl Default for StrategyConfig {
    fn default() -> Self {
        Self {
            tracked_symbols: default_symbols(),
            min_open_apr_pct: default_min_apr(),
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
    #[serde(default = "default_max_holding_hours")]
    pub max_holding_hours: f64,
    #[serde(default = "default_exit_apr")]
    pub min_exit_apr_pct: f64,
}

fn default_max_delta_pct() -> f64 {
    5.0
}
fn default_min_margin_ratio() -> f64 {
    20.0
}
fn default_max_total_notional() -> f64 {
    5000.0
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_delta_drift_pct: default_max_delta_pct(),
            min_margin_ratio_pct: default_min_margin_ratio(),
            max_total_notional_usd: default_max_total_notional(),
            auto_rebalance_delta: default_true(),
            stop_loss_basis_bps: default_stop_loss_basis_bps(),
            max_holding_hours: default_max_holding_hours(),
            min_exit_apr_pct: default_exit_apr(),
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

    pub fn default_config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config/bhyper/config.toml")
    }
}
