use crate::binance::BinanceFuturesClient;
use crate::config::Config;
use crate::hyperliquid::HyperliquidClient;
use crate::strategy::ArbitrageScanner;
use crate::telemetry::TelemetryNotifier;
use crate::ws::{BinanceWsStream, HyperliquidWsStream, MarketDataCache};
use anyhow::Result;

pub async fn run(config: &Config, limit: usize) -> Result<()> {
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
    let scanner = ArbitrageScanner::new(bn_client, hl_client, config.strategy.maker_taker_mode)
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
