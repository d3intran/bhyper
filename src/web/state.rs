use crate::config::Config;
use crate::paper::PaperTradingStore;
use crate::state::StateStore;
use crate::ws::MarketDataCache;
use arc_swap::ArcSwap;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct AppState {
    pub config: Arc<ArcSwap<Config>>,
    pub config_path: PathBuf,
    pub state_store: Arc<Mutex<StateStore>>,
    pub paper_store: Arc<Mutex<Option<PaperTradingStore>>>,
    pub market_cache: MarketDataCache,
    pub ws_broadcast: broadcast::Sender<String>,
    pub start_time: DateTime<Utc>,
}

impl AppState {
    pub fn new(
        config: Config,
        config_path: PathBuf,
        state_store: Arc<Mutex<StateStore>>,
        paper_store_opt: Option<PaperTradingStore>,
        market_cache: MarketDataCache,
    ) -> Self {
        let (tx, _) = broadcast::channel(1000);
        Self {
            config: Arc::new(ArcSwap::from_pointee(config)),
            config_path,
            state_store,
            paper_store: Arc::new(Mutex::new(paper_store_opt)),
            market_cache,
            ws_broadcast: tx,
            start_time: Utc::now(),
        }
    }

    /// Atomically updates and persists configuration without restarting
    pub fn update_config(&self, new_config: Config) -> anyhow::Result<()> {
        new_config.save_to(&self.config_path)?;
        self.config.store(Arc::new(new_config));
        Ok(())
    }
}
