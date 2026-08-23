use crate::config::Config;
use crate::paper::PaperTradingStore;
use crate::state::StateStore;
use crate::web::start_web_server;
use crate::ws::{BinanceWsStream, HyperliquidWsStream, MarketDataCache};
use anyhow::Result;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

pub async fn run(
    mut config: Config,
    config_path: PathBuf,
    state_store: Arc<Mutex<StateStore>>,
    host_override: Option<String>,
    port_override: Option<u16>,
) -> Result<()> {
    if let Some(h) = host_override {
        config.web.host = h;
    }
    if let Some(p) = port_override {
        config.web.port = p;
    }

    info!("🚀 Initializing high-speed market data cache & Web feeds...");
    let cache = MarketDataCache::new();

    // Spawn Binance WebSocket live feed
    BinanceWsStream::spawn(cache.clone());

    // Spawn Hyperliquid WebSocket live feed
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

    // Initial seed & background refresher for complete cross-exchange universe (all 200+ symbols)
    let bn_cfg = config.binance.clone();
    let hl_cfg = config.hyperliquid.clone();
    let cache_seed = cache.clone();

    tokio::spawn(async move {
        let bn_client = crate::binance::BinanceFuturesClient::new(
            bn_cfg.api_key,
            bn_cfg.api_secret,
            bn_cfg.base_url,
        );
        let hl_client = crate::hyperliquid::HyperliquidClient::new(
            hl_cfg.private_key,
            hl_cfg.wallet_address,
            hl_cfg.base_url,
        );

        // 1. Instant initial seed
        if let Ok((meta, ctxs)) = hl_client.fetch_meta_and_contexts().await {
            let mut hl_rates = Vec::with_capacity(meta.universe.len());
            let mut ois = std::collections::HashMap::with_capacity(meta.universe.len());
            for (u, ctx) in meta.universe.iter().zip(ctxs.iter()) {
                let sym = u.name.to_ascii_uppercase();
                let mark_p = ctx.mark_px.parse::<f64>().unwrap_or(0.0);
                let oracle_p = ctx
                    .oracle_px
                    .as_deref()
                    .and_then(|p| p.parse::<f64>().ok())
                    .unwrap_or(mark_p);
                let rate_1h = ctx.funding.parse::<f64>().unwrap_or(0.0);
                let apr = rate_1h * 8760.0 * 100.0;
                let oi = ctx.open_interest.parse::<f64>().unwrap_or(0.0) * mark_p;
                ois.insert(sym.clone(), oi);
                hl_rates.push(crate::types::FundingRateInfo {
                    symbol: sym,
                    exchange: crate::types::Exchange::Hyperliquid,
                    mark_price: mark_p,
                    index_price: oracle_p,
                    funding_rate: rate_1h,
                    funding_interval_hours: 1.0,
                    annualized_apr_pct: apr,
                    next_funding_time: Some(chrono::Utc::now()),
                });
            }
            cache_seed.update_hyperliquid_rates(hl_rates);
            cache_seed.update_metadata(
                std::collections::HashMap::new(),
                ois,
                std::collections::HashMap::new(),
            );
            info!(
                "🌱 Seeded Hyperliquid cache with {} universe assets",
                meta.universe.len()
            );
        }

        if let Ok(vols) = bn_client.fetch_24h_volumes().await {
            cache_seed.update_metadata(
                vols,
                std::collections::HashMap::new(),
                std::collections::HashMap::new(),
            );
        }

        // 2. Periodic background refresher (every 3 seconds) to ensure real-time accuracy across all symbols
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
        loop {
            interval.tick().await;
            if let Ok((meta, ctxs)) = hl_client.fetch_meta_and_contexts().await {
                let mut hl_rates = Vec::with_capacity(meta.universe.len());
                let mut ois = std::collections::HashMap::with_capacity(meta.universe.len());
                for (u, ctx) in meta.universe.iter().zip(ctxs.iter()) {
                    let sym = u.name.to_ascii_uppercase();
                    let mark_p = ctx.mark_px.parse::<f64>().unwrap_or(0.0);
                    let oracle_p = ctx
                        .oracle_px
                        .as_deref()
                        .and_then(|p| p.parse::<f64>().ok())
                        .unwrap_or(mark_p);
                    let rate_1h = ctx.funding.parse::<f64>().unwrap_or(0.0);
                    let apr = rate_1h * 8760.0 * 100.0;
                    let oi = ctx.open_interest.parse::<f64>().unwrap_or(0.0) * mark_p;
                    ois.insert(sym.clone(), oi);
                    hl_rates.push(crate::types::FundingRateInfo {
                        symbol: sym,
                        exchange: crate::types::Exchange::Hyperliquid,
                        mark_price: mark_p,
                        index_price: oracle_p,
                        funding_rate: rate_1h,
                        funding_interval_hours: 1.0,
                        annualized_apr_pct: apr,
                        next_funding_time: Some(chrono::Utc::now()),
                    });
                }
                cache_seed.update_hyperliquid_rates(hl_rates);
                cache_seed.update_metadata(
                    std::collections::HashMap::new(),
                    ois,
                    std::collections::HashMap::new(),
                );
            }
        }
    });

    // Warm up WebSocket feeds
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    // Load or initialize paper store
    let paper_store = PaperTradingStore::load_or_create(None, 500.0).ok();

    // Setup graceful shutdown on Ctrl+C
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        info!("🛑 Received shutdown signal (Ctrl+C). Initiating graceful shutdown...");
        let _ = shutdown_tx.send(());
    });

    start_web_server(
        config,
        config_path,
        state_store,
        paper_store,
        cache,
        Some(shutdown_rx),
    )
    .await
}
