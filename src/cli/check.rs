use crate::binance::BinanceFuturesClient;
use crate::config::Config;
use crate::hyperliquid::HyperliquidClient;
use anyhow::Result;

pub async fn run(config: &Config) -> Result<()> {
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

    Ok(())
}
