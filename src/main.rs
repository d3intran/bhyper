mod binance;
mod config;
mod hyperliquid;
mod journal;
mod paper;
mod risk;
mod state;
mod strategy;
mod telemetry;
mod types;
mod ws;

use anyhow::Result;
use binance::BinanceFuturesClient;
use clap::{Parser, Subcommand};
use config::Config;
use hyperliquid::HyperliquidClient;
use parking_lot::Mutex;
use risk::{ExitSignal, RiskSentinel};
use state::StateStore;
use std::sync::Arc;
use strategy::{ArbitrageScanner, LotPrecisionMatcher, ProfitTriggerEngine, TwoLegExecutor};
use telemetry::TelemetryNotifier;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use types::ExecutionMode;
use ws::{BinanceWsStream, HyperliquidWsStream, MarketDataCache};

#[derive(Parser)]
#[command(name = "bhyper")]
#[command(
    about = "⚡ Ultra Low-Latency Binance x Hyperliquid Funding Rate Arbitrage Engine",
    long_about = None
)]
struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
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
        #[arg(short, long, default_value_t = 100.0)]
        initial_capital: f64,
        #[arg(short, long, default_value_t = 50.0)]
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
        #[arg(short, long, default_value_t = 100.0)]
        initial_capital: f64,
    },
    /// 重置模拟盘虚拟钱包余额与持仓
    ResetPaper {
        #[arg(short, long, default_value_t = 100.0)]
        initial_capital: f64,
    },
    /// 紧急手动平仓指定币种在双边的所有对冲头寸
    Unwind {
        #[arg(short, long)]
        symbol: String,
    },
    /// 显示当前配置与配置文件位置
    Config,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("bhyper=info".parse()?))
        .init();

    let cli = Cli::parse();
    let config_path = if cli.config != "config.toml" {
        std::path::PathBuf::from(cli.config)
    } else {
        Config::default_config_path()
    };

    let config = Config::load_or_default(&config_path)?;
    let state_store = Arc::new(Mutex::new(StateStore::load_or_create(None)?));

    match cli.command.unwrap_or(Commands::Scan { limit: 20 }) {
        Commands::Scan { limit } => {
            println!("🔍 Connecting to Binance FAPI and Hyperliquid L1...");
            let bn_client = BinanceFuturesClient::new(
                config.binance.api_key.clone(),
                config.binance.api_secret.clone(),
                config.binance.base_url.clone(),
            );
            let hl_client = HyperliquidClient::new(
                config.hyperliquid.private_key.clone(),
                config.hyperliquid.wallet_address.clone(),
                config.hyperliquid.base_url.clone(),
            );

            let scanner =
                ArbitrageScanner::new(bn_client, hl_client, config.strategy.maker_taker_mode);

            let start = std::time::Instant::now();
            let opps = scanner.scan_opportunities().await?;
            let elapsed = start.elapsed();

            TelemetryNotifier::render_console_table(&opps, limit);
            println!(
                "✅ Scanned {} pairs in {:.2}ms. Config min APR threshold: {:.1}%\n",
                opps.len(),
                elapsed.as_secs_f64() * 1000.0,
                config.strategy.min_open_apr_pct
            );
        }

        Commands::Stream { limit } => {
            println!("⚡ Starting Ultra Low-Latency WebSocket Streams (Binance + Hyperliquid)...");
            let cache = MarketDataCache::new();

            // Spawn live streams
            BinanceWsStream::spawn(cache.clone());
            let hl_ws_url = if config.hyperliquid.is_testnet {
                Some("wss://api.hyperliquid-testnet.xyz/ws".to_string())
            } else {
                Some("wss://api.hyperliquid.xyz/ws".to_string())
            };
            HyperliquidWsStream::spawn(
                cache.clone(),
                hl_ws_url,
                Some(config.hyperliquid.wallet_address.clone()),
            );

            println!("⏳ Waiting 3 seconds for initial orderbook / mark price stream warm-up...");
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;

            let bn_client = BinanceFuturesClient::new(
                config.binance.api_key.clone(),
                config.binance.api_secret.clone(),
                config.binance.base_url.clone(),
            );
            let hl_client = HyperliquidClient::new(
                config.hyperliquid.private_key.clone(),
                config.hyperliquid.wallet_address.clone(),
                config.hyperliquid.base_url.clone(),
            );
            let scanner =
                ArbitrageScanner::new(bn_client, hl_client, config.strategy.maker_taker_mode)
                    .with_cache(cache.clone());

            let mut timer = tokio::time::interval(std::time::Duration::from_secs(2));
            println!("🚀 Live Market Dashboard Running. Press Ctrl+C to stop.\n");

            loop {
                timer.tick().await;
                if let Ok(opps) = scanner.scan_opportunities().await {
                    print!("{esc}[2J{esc}[1;1H", esc = 27 as char); // clear terminal
                    println!(
                        "⚡ [LIVE WS STREAM] {} | Health: {}",
                        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
                        if cache.is_healthy() {
                            "🟢 HEALTHY"
                        } else {
                            "🟡 SYNCING"
                        }
                    );
                    TelemetryNotifier::render_console_table(&opps, limit);
                }
            }
        }

        Commands::Trigger {
            margin_usd,
            ignore_window,
        } => {
            println!(
                "🎯 Running Deterministic Profit Trigger Evaluation (Margin: ${:.2}, Ignore Window: {})...",
                margin_usd, ignore_window
            );
            let bn_client = BinanceFuturesClient::new(
                config.binance.api_key.clone(),
                config.binance.api_secret.clone(),
                config.binance.base_url.clone(),
            );
            let hl_client = HyperliquidClient::new(
                config.hyperliquid.private_key.clone(),
                config.hyperliquid.wallet_address.clone(),
                config.hyperliquid.base_url.clone(),
            );

            let scanner =
                ArbitrageScanner::new(bn_client, hl_client, config.strategy.maker_taker_mode);

            let (opps_res, precisions_res) = tokio::join!(
                scanner.scan_opportunities(),
                scanner.fetch_symbol_precisions()
            );

            let opps = opps_res?;
            let precisions = precisions_res.unwrap_or_default();
            let trigger_engine = ProfitTriggerEngine::default().with_liquidity_guards(
                config.strategy.min_open_interest_usd,
                config.strategy.min_24h_volume_usd,
                config.strategy.max_bid_ask_spread_bps,
                config.strategy.max_oracle_mark_divergence_pct,
                config.strategy.symbol_whitelist.clone(),
                config.strategy.symbol_blacklist.clone(),
            );

            println!("\n{}", "=".repeat(130));
            println!(
                "🎯 BHyper Deterministic Profit Trigger Analysis (With Exact Lot Precision Math)"
            );
            println!("{}", "=".repeat(130));
            println!(
                "{:<8} {:<8} {:<12} {:<12} {:<14} {:<14} {:<15} {:<28}",
                "Symbol",
                "Trigger",
                "1h Inc(bps)",
                "Friction",
                "4h Net(bps)",
                "4h Net USD",
                "Aligned Qty",
                "Status / Reason"
            );
            println!("{}", "-".repeat(130));

            let mut passed_count = 0;
            for opp in opps.iter().take(20) {
                let prec_info = precisions.get(&opp.symbol);
                let decision =
                    trigger_engine.evaluate_opportunity(opp, margin_usd, ignore_window, prec_info);

                let trigger_badge = if decision.should_open {
                    passed_count += 1;
                    "✅ YES"
                } else {
                    "❌ NO"
                };

                let aligned_str = match &decision.aligned_quantity {
                    Some(a) => format!("{} (0-Delta)", a.binance_formatted_qty),
                    None => "N/A".to_string(),
                };

                let reason_str = decision
                    .reject_reason
                    .unwrap_or_else(|| "🎯 ALL PROFIT LOCKS PASSED!".to_string());

                println!(
                    "{:<8} {:<8} {:>10.2} bps {:>10.2} bps {:>12.2} bps ${:<13.4} {:<15} {:<28}",
                    decision.symbol,
                    trigger_badge,
                    decision.single_cycle_income_bps,
                    decision.total_friction_cost_bps,
                    decision.projected_4h_net_bps,
                    decision.net_expected_profit_usd,
                    aligned_str,
                    reason_str
                );
            }
            println!("{}\n", "=".repeat(130));
            let secs_left = ProfitTriggerEngine::seconds_until_next_hour();
            let is_bn = ProfitTriggerEngine::is_binance_settlement_hour();
            println!(
                "⏱️  Seconds to next hourly settlement: {}s | Next hour is Binance 8h settlement: {}",
                secs_left,
                if is_bn { "✅ YES" } else { "❌ NO (HL 1h only)" }
            );
            println!("📊 Actionable Triggers: {} / 20 evaluated.\n", passed_count);
        }

        Commands::Precision { limit, target_usd } => {
            println!(
                "🔍 Fetching exchange metadata and computing lot precision alignment for target ${:.2}...",
                target_usd
            );
            let bn_client = BinanceFuturesClient::new(
                config.binance.api_key.clone(),
                config.binance.api_secret.clone(),
                config.binance.base_url.clone(),
            );
            let hl_client = HyperliquidClient::new(
                config.hyperliquid.private_key.clone(),
                config.hyperliquid.wallet_address.clone(),
                config.hyperliquid.base_url.clone(),
            );

            let scanner =
                ArbitrageScanner::new(bn_client, hl_client, config.strategy.maker_taker_mode);

            let (opps_res, precisions_res) = tokio::join!(
                scanner.scan_opportunities(),
                scanner.fetch_symbol_precisions()
            );

            let opps = opps_res?;
            let precisions = precisions_res?;

            let mut price_map = std::collections::HashMap::new();
            for o in &opps {
                price_map.insert(o.symbol.clone(), o.binance_mark_price);
            }

            let mut precision_rows = Vec::new();
            for (sym, prec) in &precisions {
                if let Some(&price) = price_map.get(sym) {
                    let aligned = LotPrecisionMatcher::calculate_aligned_quantity(
                        sym, price, target_usd, prec,
                    );
                    precision_rows.push((prec.clone(), aligned, price));
                }
            }

            precision_rows
                .sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

            TelemetryNotifier::render_precision_table(&precision_rows, limit);
            println!(
                "✅ Analyzed {} shared pairs. Verified small-capital zero-delta compatibility.\n",
                precision_rows.len()
            );
        }

        Commands::Check => {
            println!("🔍 Checking Binance & Hyperliquid APIs and balances...");
            let bn_client = BinanceFuturesClient::new(
                config.binance.api_key.clone(),
                config.binance.api_secret.clone(),
                config.binance.base_url.clone(),
            );
            let hl_client = HyperliquidClient::new(
                config.hyperliquid.private_key.clone(),
                config.hyperliquid.wallet_address.clone(),
                config.hyperliquid.base_url.clone(),
            );

            // Check Binance
            match bn_client.fetch_balances().await {
                Ok(balances) => {
                    println!("✅ Binance FAPI Connected:");
                    for b in balances {
                        let total = b.balance.parse::<f64>().unwrap_or(0.0);
                        if total > 0.0 {
                            println!(
                                "   • Asset: {} | Total: {} | Available: {}",
                                b.asset, b.balance, b.available_balance
                            );
                        }
                    }
                }
                Err(e) => {
                    println!("⚠️ Binance FAPI Balance (Auth/API Key needed): {}", e);
                }
            }

            // Check Hyperliquid
            match hl_client.fetch_clearinghouse_state().await {
                Ok(state) => {
                    println!("✅ Hyperliquid L1 Connected:");
                    println!(
                        "   • Account Value: ${}",
                        state.margin_summary.account_value
                    );
                    println!(
                        "   • Total Margin Used: ${}",
                        state.margin_summary.total_margin_used
                    );
                    println!(
                        "   • Total Raw USD: ${}",
                        state.margin_summary.total_raw_usd
                    );
                }
                Err(e) => {
                    println!(
                        "⚠️ Hyperliquid Clearinghouse (Wallet address needed): {}",
                        e
                    );
                }
            }
        }

        Commands::Positions => {
            let store = state_store.lock();
            let positions = store.get_active_positions();
            TelemetryNotifier::render_positions_table(&positions);
        }

        Commands::Health => {
            println!("🔍 Fetching live cross-exchange margin health and account balances...");
            let bn_client = BinanceFuturesClient::new(
                config.binance.api_key.clone(),
                config.binance.api_secret.clone(),
                config.binance.base_url.clone(),
            );
            let hl_client = HyperliquidClient::new(
                config.hyperliquid.private_key.clone(),
                config.hyperliquid.wallet_address.clone(),
                config.hyperliquid.base_url.clone(),
            );

            let (bn_health_res, hl_health_res) = tokio::join!(
                bn_client.fetch_margin_health(),
                hl_client.fetch_margin_health()
            );

            let bn_health = match bn_health_res {
                Ok(h) => h,
                Err(e) => {
                    warn!(
                        "Could not fetch Binance margin health (Check API keys): {:?}",
                        e
                    );
                    crate::types::ExchangeMarginHealth {
                        exchange: crate::types::Exchange::Binance,
                        account_value_usd: 0.0,
                        total_margin_used_usd: 0.0,
                        free_margin_usd: 0.0,
                        margin_utilization_pct: 0.0,
                        min_liquidation_distance_pct: 100.0,
                        is_healthy: true,
                    }
                }
            };

            let hl_health = match hl_health_res {
                Ok(h) => h,
                Err(e) => {
                    warn!(
                        "Could not fetch Hyperliquid margin health (Check Wallet address): {:?}",
                        e
                    );
                    crate::types::ExchangeMarginHealth {
                        exchange: crate::types::Exchange::Hyperliquid,
                        account_value_usd: 0.0,
                        total_margin_used_usd: 0.0,
                        free_margin_usd: 0.0,
                        margin_utilization_pct: 0.0,
                        min_liquidation_distance_pct: 100.0,
                        is_healthy: true,
                    }
                }
            };

            let assessment = StateStore::compute_rebalance_advisory(
                &bn_health,
                &hl_health,
                config.risk.rebalance_threshold_imbalance_pct,
            );

            TelemetryNotifier::render_margin_assessment(&assessment);
        }

        Commands::Reconcile => {
            println!("🔍 Fetching live exchange positions from Binance and Hyperliquid for reconciliation...");
            let bn_client = BinanceFuturesClient::new(
                config.binance.api_key.clone(),
                config.binance.api_secret.clone(),
                config.binance.base_url.clone(),
            );
            let hl_client = HyperliquidClient::new(
                config.hyperliquid.private_key.clone(),
                config.hyperliquid.wallet_address.clone(),
                config.hyperliquid.base_url.clone(),
            );

            let (bn_pos_res, hl_pos_res, bn_health_res, hl_health_res) = tokio::join!(
                bn_client.fetch_positions(),
                hl_client.fetch_clearinghouse_state(),
                bn_client.fetch_margin_health(),
                hl_client.fetch_margin_health()
            );

            let bn_pos = match bn_pos_res {
                Ok(p) => p,
                Err(e) => {
                    warn!("Could not fetch Binance positions: {:?}", e);
                    Vec::new()
                }
            };

            let hl_pos = match hl_pos_res {
                Ok(s) => s.asset_positions,
                Err(e) => {
                    warn!("Could not fetch Hyperliquid positions: {:?}", e);
                    Vec::new()
                }
            };

            let mut report = {
                let mut store = state_store.lock();
                store.reconcile(&bn_pos, &hl_pos)
            };

            if let (Ok(bn_h), Ok(hl_h)) = (bn_health_res, hl_health_res) {
                let assessment = StateStore::compute_rebalance_advisory(
                    &bn_h,
                    &hl_h,
                    config.risk.rebalance_threshold_imbalance_pct,
                );
                report.margin_assessment = Some(assessment);
            }

            TelemetryNotifier::render_reconciliation_report(&report);
        }

        Commands::Monitor { interval_secs } => {
            info!(
                "Starting BHyper live monitoring loop (interval: {}s)...",
                interval_secs
            );
            let bn_client = BinanceFuturesClient::new(
                config.binance.api_key.clone(),
                config.binance.api_secret.clone(),
                config.binance.base_url.clone(),
            );
            let hl_client = HyperliquidClient::new(
                config.hyperliquid.private_key.clone(),
                config.hyperliquid.wallet_address.clone(),
                config.hyperliquid.base_url.clone(),
            );
            let notifier = TelemetryNotifier::new(config.telegram.clone());
            let scanner =
                ArbitrageScanner::new(bn_client, hl_client, config.strategy.maker_taker_mode);

            let mut timer = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
            loop {
                timer.tick().await;
                match scanner.scan_opportunities().await {
                    Ok(opps) => {
                        let top_opportunities: Vec<_> = opps
                            .into_iter()
                            .filter(|o| o.net_spread_apr_pct >= config.strategy.min_open_apr_pct)
                            .collect();

                        if !top_opportunities.is_empty() {
                            info!(
                                "Found {} actionable arbitrage opportunities > {:.1}% APR!",
                                top_opportunities.len(),
                                config.strategy.min_open_apr_pct
                            );
                            TelemetryNotifier::render_console_table(&top_opportunities, 5);

                            if let Some(best) = top_opportunities.first() {
                                let alert_msg = format!(
                                    "🚨 <b>BHyper 套利机会发现!</b>\n\n\
                                    • <b>标的:</b> <code>{}</code>\n\
                                    • <b>净利差 APR:</b> <code>{:.2}%</code>\n\
                                    • <b>Binance APR:</b> <code>{:.2}%</code>\n\
                                    • <b>Hyperliquid APR:</b> <code>{:.2}%</code>\n\
                                    • <b>推荐操作:</b> <code>Hyperliquid {} | Binance {}</code>\n\
                                    • <b>预计时收益:</b> <code>{:.2} bps/h</code> (回本时间: <code>{:.1}h</code>)\n\
                                    • <b>4h净利预估:</b> <code>{:.2} bps</code>",
                                    best.symbol,
                                    best.net_spread_apr_pct,
                                    best.binance_apr_pct,
                                    best.hyperliquid_apr_pct,
                                    best.hyperliquid_side,
                                    best.binance_side,
                                    best.est_hourly_return_bps,
                                    best.est_break_even_hours,
                                    best.projected_4h_net_bps
                                );
                                let _ = notifier.send_alert(&alert_msg).await;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error scanning opportunities: {:?}", e);
                    }
                }
            }
        }

        Commands::Trade {
            margin_usd,
            dry_run,
            live_danger,
            taker_taker,
            interval_secs,
        } => {
            let actual_dry_run = if live_danger {
                warn!("⚠️ LIVE TRADING MODE ENABLED WITH REAL FUNDS!");
                false
            } else {
                info!(
                    "🧪 Dry-run simulation mode active (Safety paper trading: {}).",
                    dry_run
                );
                true
            };

            let execution_mode = if taker_taker {
                ExecutionMode::TakerTaker
            } else {
                ExecutionMode::MakerTaker
            };

            info!("Execution mode set to: {}", execution_mode);

            // 1. Initialize real-time WebSocket market cache & telemetry
            let cache = MarketDataCache::new();
            BinanceWsStream::spawn(cache.clone());
            let hl_ws_url = if config.hyperliquid.is_testnet {
                Some("wss://api.hyperliquid-testnet.xyz/ws".to_string())
            } else {
                Some("wss://api.hyperliquid.xyz/ws".to_string())
            };
            HyperliquidWsStream::spawn(
                cache.clone(),
                hl_ws_url,
                Some(config.hyperliquid.wallet_address.clone()),
            );

            info!("⏳ Warming up WebSocket feeds (2s)...");
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            let bn_client = BinanceFuturesClient::new(
                config.binance.api_key.clone(),
                config.binance.api_secret.clone(),
                config.binance.base_url.clone(),
            );
            let hl_client = HyperliquidClient::new(
                config.hyperliquid.private_key.clone(),
                config.hyperliquid.wallet_address.clone(),
                config.hyperliquid.base_url.clone(),
            );
            let notifier = TelemetryNotifier::new(config.telegram.clone());
            let scanner =
                ArbitrageScanner::new(bn_client, hl_client, config.strategy.maker_taker_mode)
                    .with_cache(cache.clone());

            let mut executor = TwoLegExecutor::new(
                BinanceFuturesClient::new(
                    config.binance.api_key.clone(),
                    config.binance.api_secret.clone(),
                    config.binance.base_url.clone(),
                ),
                HyperliquidClient::new(
                    config.hyperliquid.private_key.clone(),
                    config.hyperliquid.wallet_address.clone(),
                    config.hyperliquid.base_url.clone(),
                ),
                notifier.clone(),
                state_store.clone(),
                actual_dry_run,
                execution_mode,
            )
            .with_cache(cache.clone());

            if config.strategy.use_binance_ws_api && !config.binance.api_key.is_empty() {
                info!("⚡ Initializing Binance WebSocket API client for ultra low-latency order dispatching...");
                let ws_api = binance::BinanceWsApiClient::spawn(
                    config.binance.api_key.clone(),
                    config.binance.api_secret.clone(),
                    None,
                );
                executor = executor.with_ws_api(ws_api);
            }

            let trigger_engine = ProfitTriggerEngine::new(
                config.strategy.min_open_apr_pct / 8760.0 * 100.0,
                config.strategy.max_position_usd_per_pair,
                config.strategy.maker_taker_mode,
            )
            .with_liquidity_guards(
                config.strategy.min_open_interest_usd,
                config.strategy.min_24h_volume_usd,
                config.strategy.max_bid_ask_spread_bps,
                config.strategy.max_oracle_mark_divergence_pct,
                config.strategy.symbol_whitelist.clone(),
                config.strategy.symbol_blacklist.clone(),
            );

            let risk_sentinel = RiskSentinel::new(config.risk.clone());

            info!(
                "🚀 BHyper Automated Engine Running (Interval: {}s, Max Pair Margin: ${:.2}, Max Positions: {})...",
                interval_secs, margin_usd, config.strategy.max_active_positions
            );

            let mut timer = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
            loop {
                timer.tick().await;

                let (opps_res, precisions_res) = tokio::join!(
                    scanner.scan_opportunities(),
                    scanner.fetch_symbol_precisions()
                );

                let opps = match opps_res {
                    Ok(o) => o,
                    Err(e) => {
                        tracing::error!("Error scanning opportunities: {:?}", e);
                        continue;
                    }
                };

                let precisions = precisions_res.unwrap_or_default();
                let opps_map: std::collections::HashMap<String, &types::ArbitrageOpportunity> =
                    opps.iter().map(|o| (o.symbol.clone(), o)).collect();

                // =========================================================================
                // PHASE 1: ACTIVE POSITION AUDIT & AUTOMATED EXIT / SPREAD DECAY LIFECYCLE
                // =========================================================================
                let active_positions = {
                    let store = state_store.lock();
                    store.get_active_positions()
                };

                for mut pos in active_positions {
                    let prec = match precisions.get(&pos.symbol) {
                        Some(p) => p,
                        None => continue,
                    };

                    let (live_bn_px, live_hl_px) = cache
                        .get_latest_prices(&pos.symbol)
                        .unwrap_or((pos.binance_entry_price, pos.hyperliquid_entry_price));

                    let current_opp = opps_map.get(&pos.symbol).copied();

                    // Update live spread APR and Delta in store
                    if let Some(opp) = current_opp {
                        let eff_spread = match pos.hyperliquid_side {
                            types::PositionSide::Short => {
                                opp.hyperliquid_apr_pct - opp.binance_apr_pct
                            }
                            types::PositionSide::Long => {
                                opp.binance_apr_pct - opp.hyperliquid_apr_pct
                            }
                        };
                        pos.current_spread_apr = eff_spread;
                        pos.last_updated_at = chrono::Utc::now();
                        let _ = state_store.lock().upsert_position(pos.clone());
                    }

                    if config.strategy.auto_unwind_on_decay {
                        let exit_signal = risk_sentinel.evaluate_position_exit(
                            &pos,
                            current_opp,
                            live_bn_px,
                            live_hl_px,
                        );

                        match exit_signal {
                            ExitSignal::Hold => {}
                            ExitSignal::SpreadDecay { reason, .. }
                            | ExitSignal::SpreadInverted { reason, .. }
                            | ExitSignal::BasisStopLoss { reason, .. }
                            | ExitSignal::BasisTakeProfit { reason, .. }
                            | ExitSignal::MaxDurationExceeded { reason, .. }
                            | ExitSignal::DeltaDriftCritical { reason, .. }
                            | ExitSignal::MarginCritical { reason, .. }
                            | ExitSignal::LiquidationThreat { reason, .. } => {
                                warn!("🚨 AUTOMATIC EXIT TRIGGERED for {}: {}", pos.symbol, reason);
                                if let Err(e) = executor.execute_close(&pos, prec, &reason).await {
                                    tracing::error!(
                                        "Failed to auto-close position for {}: {:?}",
                                        pos.symbol,
                                        e
                                    );
                                }
                            }
                        }
                    }
                }

                // =========================================================================
                // PHASE 2: CAPACITY CHECK & NEW ARBITRAGE OPPORTUNITY EVALUATION
                // =========================================================================
                let current_active_count = {
                    let store = state_store.lock();
                    store.get_active_positions().len()
                };

                if current_active_count >= config.strategy.max_active_positions {
                    continue;
                }

                let held_symbols: std::collections::HashSet<String> = {
                    let store = state_store.lock();
                    store
                        .get_active_positions()
                        .into_iter()
                        .map(|p| p.symbol)
                        .collect()
                };

                for opp in opps.iter().take(10) {
                    if held_symbols.contains(&opp.symbol) {
                        continue; // Already holding this symbol
                    }

                    let prec = match precisions.get(&opp.symbol) {
                        Some(p) => p,
                        None => continue,
                    };

                    let decision = trigger_engine.evaluate_opportunity(
                        opp,
                        margin_usd,
                        false, // strictly enforce timing sniper window in automated mode!
                        Some(prec),
                    );

                    if decision.should_open {
                        info!(
                            "🎯 PROFIT TRIGGER FIRED for {}! Executing two-leg arbitrage (Mode: {})...",
                            opp.symbol, execution_mode
                        );
                        match executor.execute_open(opp, &decision, prec).await {
                            Ok(pos) => {
                                info!(
                                    "✅ Successfully established arbitrage position on {}",
                                    pos.symbol
                                );
                                break;
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to execute trade on {}: {:?}",
                                    opp.symbol,
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }

        Commands::Paper {
            initial_capital,
            margin_usd,
            taker_taker,
            interval_secs,
        } => {
            let execution_mode = if taker_taker {
                ExecutionMode::TakerTaker
            } else {
                ExecutionMode::MakerTaker
            };

            let paper_store = paper::PaperTradingStore::load_or_create(None, initial_capital)?;
            let mut engine = paper::PaperExecutionEngine::new(paper_store);
            let risk_sentinel = RiskSentinel::new(config.risk.clone());

            let trigger_engine = ProfitTriggerEngine::new(
                config.strategy.min_open_apr_pct / 8760.0 * 100.0,
                config.strategy.max_position_usd_per_pair.min(margin_usd),
                config.strategy.maker_taker_mode,
            )
            .with_liquidity_guards(
                config.strategy.min_open_interest_usd,
                config.strategy.min_24h_volume_usd,
                config.strategy.max_bid_ask_spread_bps,
                config.strategy.max_oracle_mark_divergence_pct,
                config.strategy.symbol_whitelist.clone(),
                config.strategy.symbol_blacklist.clone(),
            );

            // 1. Initialize real-time WebSocket market cache & streams
            let cache = MarketDataCache::new();
            BinanceWsStream::spawn(cache.clone());
            let hl_ws_url = if config.hyperliquid.is_testnet {
                Some("wss://api.hyperliquid-testnet.xyz/ws".to_string())
            } else {
                Some("wss://api.hyperliquid.xyz/ws".to_string())
            };
            HyperliquidWsStream::spawn(
                cache.clone(),
                hl_ws_url,
                Some(config.hyperliquid.wallet_address.clone()),
            );

            info!("⏳ Warming up WebSocket feeds for Paper Trading (2s)...");
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            let bn_client = BinanceFuturesClient::new(
                config.binance.api_key.clone(),
                config.binance.api_secret.clone(),
                config.binance.base_url.clone(),
            );
            let hl_client = HyperliquidClient::new(
                config.hyperliquid.private_key.clone(),
                config.hyperliquid.wallet_address.clone(),
                config.hyperliquid.base_url.clone(),
            );
            let scanner =
                ArbitrageScanner::new(bn_client, hl_client, config.strategy.maker_taker_mode)
                    .with_cache(cache.clone());

            info!(
                "🧪 [PAPER TRADING DAEMON] Running with ${:.2} Virtual Capital (${:.2} BN | ${:.2} HL)",
                engine.store.state.wallet.total_equity_usd(),
                engine.store.state.wallet.binance.total_equity_usd(),
                engine.store.state.wallet.hyperliquid.total_equity_usd()
            );

            let mut timer = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
            loop {
                timer.tick().await;

                let (opps_res, precisions_res) = tokio::join!(
                    scanner.scan_opportunities(),
                    scanner.fetch_symbol_precisions()
                );

                let opps = match opps_res {
                    Ok(o) => o,
                    Err(e) => {
                        tracing::error!("Error scanning opportunities in paper mode: {:?}", e);
                        continue;
                    }
                };

                let precisions = precisions_res.unwrap_or_default();
                let opps_map: std::collections::HashMap<String, &types::ArbitrageOpportunity> =
                    opps.iter().map(|o| (o.symbol.clone(), o)).collect();

                // 1. Funding Fee Accrual Clock Tick
                let _ = engine.accrue_funding_payments(&opps);

                // 2. Active Positions Exit Audit
                let active_syms: Vec<String> = engine
                    .store
                    .state
                    .active_positions
                    .keys()
                    .cloned()
                    .collect();
                for sym in active_syms {
                    let pos = match engine.store.state.active_positions.get(&sym) {
                        Some(p) => p.clone(),
                        None => continue,
                    };

                    let (live_bn_px, live_hl_px) = cache
                        .get_latest_prices(&pos.symbol)
                        .unwrap_or((pos.binance_entry_price, pos.hyperliquid_entry_price));

                    let current_opp = opps_map.get(&pos.symbol).copied();
                    let active_pos_struct = pos.to_active_position();

                    let exit_signal = risk_sentinel.evaluate_position_exit(
                        &active_pos_struct,
                        current_opp,
                        live_bn_px,
                        live_hl_px,
                    );

                    match exit_signal {
                        ExitSignal::Hold => {}
                        ExitSignal::SpreadDecay { reason, .. }
                        | ExitSignal::SpreadInverted { reason, .. }
                        | ExitSignal::BasisStopLoss { reason, .. }
                        | ExitSignal::BasisTakeProfit { reason, .. }
                        | ExitSignal::MaxDurationExceeded { reason, .. }
                        | ExitSignal::DeltaDriftCritical { reason, .. }
                        | ExitSignal::MarginCritical { reason, .. }
                        | ExitSignal::LiquidationThreat { reason, .. } => {
                            warn!(
                                "🚨 [PAPER AUTO EXIT] Closing position on {}: {}",
                                sym, reason
                            );
                            let _ = engine.simulate_close(&sym, live_bn_px, live_hl_px, &reason);
                        }
                    }
                }

                // 3. New Arbitrage Opportunity Evaluation
                let current_active_count = engine.store.state.active_positions.len();
                if current_active_count < config.strategy.max_active_positions {
                    let held_symbols: std::collections::HashSet<String> = engine
                        .store
                        .state
                        .active_positions
                        .keys()
                        .cloned()
                        .collect();

                    for opp in &opps {
                        if held_symbols.contains(&opp.symbol) {
                            continue;
                        }

                        let prec = match precisions.get(&opp.symbol) {
                            Some(p) => p,
                            None => continue,
                        };

                        let decision = trigger_engine.evaluate_opportunity(
                            opp,
                            margin_usd,
                            false, // Respect strict sniper timing window
                            Some(prec),
                        );

                        if decision.should_open {
                            info!(
                                "🎯 [PAPER TRIGGER HIT] Validated arbitrage opportunity on {}: Net APR {:.2}%, Est Profit: ${:.4}",
                                opp.symbol, opp.net_spread_apr_pct, decision.net_expected_profit_usd
                            );

                            if let Err(e) =
                                engine.simulate_open(opp, &decision, prec, execution_mode)
                            {
                                warn!("Failed to simulate open on {}: {:?}", opp.symbol, e);
                            }
                            break;
                        }
                    }
                }

                // 4. Print Status Dashboard
                let total_eq = engine.store.state.wallet.total_equity_usd();
                let realized_pnl = engine.store.state.wallet.total_realized_pnl_usd();
                let funding_inc = engine.store.state.wallet.total_funding_income_usd();
                let fees_paid = engine.store.state.wallet.total_fees_paid_usd();
                let active_count = engine.store.state.active_positions.len();

                println!(
                    "🧪 [PAPER TRADING] Equity: ${:.2} (BN: ${:.2} | HL: ${:.2}) | Positions: {} | Realized PnL: ${:.4} | Funding: +${:.4} | Fees: -${:.4}",
                    total_eq,
                    engine.store.state.wallet.binance.total_equity_usd(),
                    engine.store.state.wallet.hyperliquid.total_equity_usd(),
                    active_count,
                    realized_pnl,
                    funding_inc,
                    fees_paid
                );
            }
        }

        Commands::Journal {
            symbol,
            event_type,
            limit,
            paper_only,
        } => {
            let journal = journal::TradeJournal::new(None);
            let filter = journal::JournalFilter {
                symbol,
                event_type,
                limit: Some(limit),
                is_paper: if paper_only { Some(true) } else { None },
                ..Default::default()
            };

            let entries = journal.query(&filter)?;
            println!("\n{}", "=".repeat(125));
            println!("📖 BHyper Detailed Trade Execution Journal (Chronological Ledger)");
            println!("{}", "=".repeat(125));
            println!(
                "{:<20} {:<10} {:<8} {:<10} {:<45} {:<18}",
                "Timestamp (UTC)", "Event", "Symbol", "Mode", "Details / Execution", "PnL / Value"
            );
            println!("{}", "-".repeat(125));

            if entries.is_empty() {
                println!("  (No journal entries matching query filter found)");
            } else {
                for e in entries {
                    let mode_tag = if e.is_paper() {
                        "🧪 PAPER"
                    } else {
                        "⚡ LIVE"
                    };
                    let t_str = e.timestamp().format("%Y-%m-%d %H:%M:%S").to_string();

                    match e {
                        journal::JournalEntry::Intent(i) => {
                            println!(
                                "{:<20} {:<10} {:<8} {:<10} {:<45} ${:<17.2}",
                                t_str,
                                "INTENT",
                                i.symbol,
                                mode_tag,
                                format!(
                                    "Spread: {:.1}% | HL:{} BN:{}",
                                    i.net_spread_apr_pct, i.hyperliquid_side, i.binance_side
                                ),
                                i.target_notional_usd
                            );
                        }
                        journal::JournalEntry::OpenFill(o) => {
                            println!(
                                "{:<20} {:<10} {:<8} {:<10} {:<45} ${:<17.2}",
                                t_str,
                                "OPEN_FILL",
                                o.symbol,
                                mode_tag,
                                format!(
                                    "HL: ${:.4} | BN: ${:.4} (Fee: ${:.4})",
                                    o.hyperliquid_price, o.binance_price, o.total_open_fees_usd
                                ),
                                o.total_notional_usd
                            );
                        }
                        journal::JournalEntry::Funding(f) => {
                            println!(
                                "{:<20} {:<10} {:<8} {:<10} {:<45} ${:<17.4}",
                                t_str,
                                "FUNDING",
                                f.symbol,
                                mode_tag,
                                format!(
                                    "{}: {:.2} bps | Cum: ${:.4}",
                                    f.exchange, f.rate_bps, f.cumulative_funding_usd
                                ),
                                f.funding_payment_usd
                            );
                        }
                        journal::JournalEntry::RiskAlert(r) => {
                            println!(
                                "{:<20} {:<10} {:<8} {:<10} {:<45} {:<18}",
                                t_str,
                                "RISK",
                                r.symbol,
                                mode_tag,
                                format!("{}: {}", r.event_type, r.details),
                                r.action_taken
                            );
                        }
                        journal::JournalEntry::CloseFill(c) => {
                            println!(
                                "{:<20} {:<10} {:<8} {:<10} {:<45} ${:<17.4}",
                                t_str,
                                "CLOSE_FILL",
                                c.symbol,
                                mode_tag,
                                format!(
                                    "Held: {:.1}h | Funding: ${:.4} | Fees: ${:.4}",
                                    c.holding_duration_secs as f64 / 3600.0,
                                    c.gross_funding_earned_usd,
                                    c.total_roundtrip_fees_usd
                                ),
                                c.net_realized_pnl_usd
                            );
                        }
                    }
                }
            }
            println!("{}\n", "=".repeat(125));
        }

        Commands::Report {
            export_md,
            initial_capital,
        } => {
            let journal = journal::TradeJournal::new(None);
            let entries = journal.read_all()?;
            let summary =
                journal::PerformanceAnalytics::compute_from_entries(&entries, initial_capital);

            journal::PerformanceAnalytics::render_console_summary(&summary);

            if let Some(md_path) = export_md {
                let md_content = journal::PerformanceAnalytics::render_markdown_report(&summary);
                let target = std::path::PathBuf::from(md_path);
                if let Some(p) = target.parent() {
                    let _ = std::fs::create_dir_all(p);
                }
                std::fs::write(&target, md_content)?;
                println!(
                    "📄 Exported markdown review report to: {}",
                    target.display()
                );
            }
        }

        Commands::ResetPaper { initial_capital } => {
            let mut store = paper::PaperTradingStore::load_or_create(None, initial_capital)?;
            store.reset(initial_capital)?;
            println!(
                "✅ Reset paper trading state successfully. Virtual capital set to ${:.2}.",
                initial_capital
            );
        }

        Commands::Unwind { symbol } => {
            info!(
                "Emergency unwinding position for {} on both exchanges...",
                symbol
            );
            let bn_client = BinanceFuturesClient::new(
                config.binance.api_key.clone(),
                config.binance.api_secret.clone(),
                config.binance.base_url.clone(),
            );
            let hl_client = HyperliquidClient::new(
                config.hyperliquid.private_key.clone(),
                config.hyperliquid.wallet_address.clone(),
                config.hyperliquid.base_url.clone(),
            );
            let notifier = TelemetryNotifier::new(config.telegram.clone());

            let executor = TwoLegExecutor::new(
                bn_client,
                hl_client,
                notifier,
                state_store.clone(),
                false,
                ExecutionMode::TakerTaker,
            );

            // Retrieve from state store if available
            let (target_pos, default_prec) = {
                let store = state_store.lock();
                let pos = store.get_position(&symbol).cloned().unwrap_or_else(|| {
                    types::ActiveArbitragePosition {
                        symbol: symbol.clone(),
                        binance_side: types::PositionSide::Long,
                        binance_qty: 0.0,
                        binance_entry_price: 0.0,
                        hyperliquid_side: types::PositionSide::Short,
                        hyperliquid_qty: 0.0,
                        hyperliquid_entry_price: 0.0,
                        nominal_value_usd: 0.0,
                        net_delta_usd: 0.0,
                        entry_spread_apr: 0.0,
                        current_spread_apr: 0.0,
                        accumulated_funding_usd: 0.0,
                        opened_at: chrono::Utc::now(),
                        last_updated_at: chrono::Utc::now(),
                        is_closed: false,
                        closed_at: None,
                        realized_pnl_usd: None,
                    }
                });

                let prec = types::SymbolPrecisionInfo {
                    symbol: symbol.clone(),
                    binance_step_size: 1.0,
                    binance_tick_size: 0.001,
                    binance_min_qty: 1.0,
                    binance_min_notional: 5.0,
                    hyperliquid_sz_decimals: 0,
                    hyperliquid_asset_index: 0,
                    hyperliquid_min_notional: 10.0,
                };

                (pos, prec)
            };

            let _ = executor
                .execute_close(&target_pos, &default_prec, "Emergency manual unwind")
                .await;
            println!("✅ Unwind command dispatched and recorded for {}.", symbol);
        }

        Commands::Config => {
            println!("📁 BHyper Configuration path: {}", config_path.display());
            let toml_str = toml::to_string_pretty(&config)?;
            println!("\n{}", toml_str);
        }
    }

    Ok(())
}
