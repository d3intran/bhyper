mod binance;
mod config;
mod hyperliquid;
mod risk;
mod strategy;
mod telemetry;
mod types;

use anyhow::Result;
use binance::BinanceFuturesClient;
use clap::{Parser, Subcommand};
use config::Config;
use hyperliquid::HyperliquidClient;
use strategy::ArbitrageScanner;
use telemetry::TelemetryNotifier;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "bhyper")]
#[command(about = "⚡ Ultra Low-Latency Binance x Hyperliquid Funding Rate Arbitrage Engine", long_about = None)]
struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 实时扫描并展示币安与 Hyperliquid 全币种资金费率利差排行
    Scan {
        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },
    /// 启动实时利差监控与 Telegram 预警守护循环
    Monitor {
        #[arg(short, long, default_value_t = 15)]
        interval_secs: u64,
    },
    /// 检查并验证 Binance 与 Hyperliquid API 连接与账户权益
    Check,
    /// 运行确定性盈利扳机评估器 (单次套利可行性与窗口校验)
    Trigger {
        #[arg(short, long, default_value_t = 50.0)]
        margin_usd: f64,
        #[arg(short, long, default_value_t = false)]
        ignore_window: bool,
    },
    /// 显示当前配置与配置文件位置
    Config,
}

#[tokio::main]
async fn main() -> Result<()> {
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
                                    • <b>预计时收益:</b> <code>{:.2} bps/h</code> (回本时间: <code>{:.1}h</code>)",
                                    best.symbol,
                                    best.net_spread_apr_pct,
                                    best.binance_apr_pct,
                                    best.hyperliquid_apr_pct,
                                    best.hyperliquid_side,
                                    best.binance_side,
                                    best.est_hourly_return_bps,
                                    best.est_break_even_hours
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

        Commands::Trigger {
            margin_usd,
            ignore_window,
        } => {
            println!("🎯 Running Deterministic Profit Trigger Evaluation (Margin: ${:.2}, Ignore Window: {})...", margin_usd, ignore_window);
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

            let trigger_engine = strategy::ProfitTriggerEngine::default();
            let opps = scanner.scan_opportunities().await?;

            println!("\n{}", "=".repeat(110));
            println!("🎯 BHyper Deterministic Profit Trigger Analysis (Small Capital Guard: $100 Framework)");
            println!("{}", "=".repeat(110));
            println!(
                "{:<8} {:<8} {:<12} {:<12} {:<12} {:<15} {:<30}",
                "Symbol",
                "Trigger",
                "1h Inc(bps)",
                "Friction",
                "Net PnL",
                "Notional",
                "Status / Reason"
            );
            println!("{}", "-".repeat(110));

            let mut passed_count = 0;
            for opp in opps.iter().take(15) {
                let decision = trigger_engine.evaluate_opportunity(opp, margin_usd, ignore_window);
                let trigger_badge = if decision.should_open {
                    passed_count += 1;
                    "✅ YES"
                } else {
                    "❌ NO"
                };

                let reason_str = decision
                    .reject_reason
                    .unwrap_or_else(|| "🎯 ALL PROFIT LOCKS PASSED!".to_string());

                println!(
                    "{:<8} {:<8} {:>10.2} bps {:>10.2} bps {:>10.2} bps ${:<14.2} {:<30}",
                    decision.symbol,
                    trigger_badge,
                    decision.single_cycle_income_bps,
                    decision.total_friction_cost_bps,
                    decision.net_expected_profit_bps,
                    decision.target_notional_usd,
                    reason_str
                );
            }
            println!("{}\n", "=".repeat(110));
            let secs_left = strategy::ProfitTriggerEngine::seconds_until_next_hour();
            println!(
                "⏱️  Seconds to next hourly settlement: {}s (Sniper window: 10s ~ 60s)",
                secs_left
            );
            println!("📊 Actionable Triggers: {} / 15 evaluated.\n", passed_count);
        }

        Commands::Config => {
            println!("📁 BHyper Configuration path: {}", config_path.display());
            let toml_str = toml::to_string_pretty(&config)?;
            println!("\n{}", toml_str);
        }
    }

    Ok(())
}
