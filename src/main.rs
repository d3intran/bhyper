use anyhow::Result;
use bhyper::cli::{run_cli, Cli, Commands};
use bhyper::config::Config;
use bhyper::state::StateStore;
use clap::Parser;
use parking_lot::Mutex;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

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

    let command = cli.command.unwrap_or(Commands::Scan { limit: 20 });
    run_cli(command, &config, config_path, state_store).await
}
