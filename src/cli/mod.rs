pub mod check;
pub mod health;
pub mod journal;
pub mod monitor;
pub mod paper;
pub mod positions;
pub mod precision;
pub mod reconcile;
pub mod report;
pub mod scan;
pub mod stream;
pub mod trade;
pub mod trigger;
pub mod unwind;

use crate::config::Config;
use crate::state::StateStore;
use anyhow::Result;
use clap::{Parser, Subcommand};
use parking_lot::Mutex;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "bhyper")]
#[command(
    about = "⚡ Ultra Low-Latency Binance x Hyperliquid Funding Rate Arbitrage Engine",
    long_about = None
)]
pub struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    pub config: String,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 实时扫描并展示币安与 Hyperliquid 全币种资金费率利差排行 (含多持仓周期净利测算)
    Scan {
        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },
    /// 启动亚毫秒级实时 WebSocket 行情数据流与利差仪表盘
    Stream {
        #[arg(short, long, default_value_t = 15)]
        limit: usize,
    },
    /// 运行确定性盈利扳机评估器 (单次套利可行性、精确步长对齐与整点窗口校验)
    Trigger {
        #[arg(short, long, default_value_t = 50.0)]
        margin_usd: f64,
        #[arg(short, long, default_value_t = false)]
        ignore_window: bool,
    },
    /// 检查两所所有共同支持交易对的下单步长精度与小资金对齐可行性
    Precision {
        #[arg(short, long, default_value_t = 25)]
        limit: usize,
        #[arg(short, long, default_value_t = 50.0)]
        target_usd: f64,
    },
    /// 检查并验证 Binance 与 Hyperliquid API 连接与账户权益
    Check,
    /// 查看本地持久化存储的所有活跃套利持仓
    Positions,
    /// 跨所账户保证金健康度与资金再平衡建议 (Margin Health & Capital Rebalance Advisory)
    Health,
    /// 跨所持仓对账审计 (自动核对 Binance / Hyperliquid 真实头寸与本地记录，检测孤儿腿)
    Reconcile,
    /// 启动实时利差监控与 Telegram 预警守护循环
    Monitor {
        #[arg(short, long, default_value_t = 10)]
        interval_secs: u64,
    },
    /// 启动套利执行引擎 (支持安全模拟盘 Paper Trading 与实盘成交校验两腿对冲)
    Trade {
        #[arg(short, long, default_value_t = 50.0)]
        margin_usd: f64,
        #[arg(short, long, default_value_t = true)]
        dry_run: bool,
        #[arg(long, default_value_t = false)]
        live_danger: bool,
        #[arg(long, default_value_t = false)]
        taker_taker: bool,
        #[arg(short, long, default_value_t = 5)]
        interval_secs: u64,
    },
    /// 启动高严谨度虚拟模拟盘守护引擎 (双所虚拟钱包、全息滑点与手续费扣除、每小时真实资金费流水记账)
    Paper {
        #[arg(short, long, default_value_t = 500.0)]
        initial_capital: f64,
        #[arg(short, long, default_value_t = 100.0)]
        margin_usd: f64,
        #[arg(long, default_value_t = false)]
        taker_taker: bool,
        #[arg(short, long, default_value_t = 5)]
        interval_secs: u64,
    },
    /// 查看并筛选全息交易流水日志 (Trade Intent, Fills, Funding Accruals, Risk Audits)
    Journal {
        #[arg(short, long)]
        symbol: Option<String>,
        #[arg(short, long)]
        event_type: Option<String>,
        #[arg(short, long, default_value_t = 30)]
        limit: usize,
        #[arg(long)]
        paper_only: bool,
    },
    /// 生成机构级复盘与策略绩效分析报告 (Win Rate, PnL Attribution, Drawdown, Funding vs Fees)
    Report {
        #[arg(long)]
        export_md: Option<String>,
        #[arg(short, long, default_value_t = 500.0)]
        initial_capital: f64,
    },
    /// 重置模拟盘虚拟钱包余额与持仓
    ResetPaper {
        #[arg(short, long, default_value_t = 500.0)]
        initial_capital: f64,
    },
    /// 手动执行单次指定币种模拟开仓或平仓测试 (Manual Single-Shot Paper Trade Test)
    PaperTrade {
        #[arg(short, long)]
        symbol: String,
        #[arg(short, long, default_value_t = 50.0)]
        margin_usd: f64,
        #[arg(short, long, default_value = "open")]
        action: String,
    },
    /// 紧急手动平仓指定币种在双边的所有对冲头寸
    Unwind {
        #[arg(short, long)]
        symbol: String,
    },
    /// 显示当前配置与配置文件位置
    Config,
}

pub async fn run_cli(
    command: Commands,
    config: &Config,
    state_store: Arc<Mutex<StateStore>>,
) -> Result<()> {
    match command {
        Commands::Scan { limit } => scan::run(config, limit).await,
        Commands::Stream { limit } => stream::run(config, limit).await,
        Commands::Trigger {
            margin_usd,
            ignore_window,
        } => trigger::run(config, margin_usd, ignore_window).await,
        Commands::Precision { limit, target_usd } => {
            precision::run(config, limit, target_usd).await
        }
        Commands::Check => check::run(config).await,
        Commands::Positions => {
            positions::run(state_store);
            Ok(())
        }
        Commands::Health => health::run(config).await,
        Commands::Reconcile => reconcile::run(config, state_store).await,
        Commands::Monitor { interval_secs } => monitor::run(config, interval_secs).await,
        Commands::Trade {
            margin_usd,
            dry_run,
            live_danger,
            taker_taker,
            interval_secs,
        } => {
            trade::run(
                config,
                state_store,
                margin_usd,
                dry_run,
                live_danger,
                taker_taker,
                interval_secs,
            )
            .await
        }
        Commands::Paper {
            initial_capital,
            margin_usd,
            taker_taker,
            interval_secs,
        } => {
            paper::run_daemon(
                config,
                initial_capital,
                margin_usd,
                taker_taker,
                interval_secs,
            )
            .await
        }
        Commands::Journal {
            symbol,
            event_type,
            limit,
            paper_only,
        } => journal::run(symbol, event_type, limit, paper_only),
        Commands::Report {
            export_md,
            initial_capital,
        } => report::run(export_md, initial_capital),
        Commands::ResetPaper { initial_capital } => paper::run_reset(initial_capital),
        Commands::PaperTrade {
            symbol,
            margin_usd,
            action,
        } => paper::run_trade(config, &symbol, margin_usd, &action).await,
        Commands::Unwind { symbol } => unwind::run(config, state_store, &symbol).await,
        Commands::Config => {
            let path = Config::default_config_path();
            println!("⚙️ Configuration file: {}", path.display());
            println!("--------------------------------------------------");
            println!("{}", toml::to_string_pretty(config)?);
            Ok(())
        }
    }
}
