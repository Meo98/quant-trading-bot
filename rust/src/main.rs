//! Matrix Quant Core - Standalone Daemon Mode
//!
//! This binary runs the trading engine as a standalone daemon process.
//! It reads configuration from config.json and runs the trading loop
//! indefinitely with 60-second tick intervals.
//!
//! # Usage
//!
//! ```bash
//! cd rust && cargo build --release
//! ./target/release/matrix-quant-core
//! ```

use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use tokio::time::{sleep, Duration};

mod api;
mod config;
mod trading;

use config::BotConfig;
use trading::engine::TradingEngine;

/// Configuration structure matching the Python config.json format
#[derive(Debug, Deserialize)]
struct ExchangeConfig {
    name: String,
    key: String,
    secret: String,
}

#[derive(Debug, Deserialize)]
struct FileConfig {
    max_open_trades: usize,
    exchange: ExchangeConfig,
    #[serde(default)]
    dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    log::info!("Matrix Quant Core v0.1.0");
    log::info!("========================");

    // Load configuration
    let config_path = Path::new("/home/meo/trading-bot/config.json");

    if !config_path.exists() {
        log::error!("Config file not found at {:?}", config_path);
        log::info!("Please create config.json with your Kraken API credentials.");
        std::process::exit(1);
    }

    let config_str = fs::read_to_string(config_path)?;
    let file_config: FileConfig = serde_json::from_str(&config_str)?;

    // Validate exchange
    if file_config.exchange.name.to_lowercase() != "kraken" {
        log::error!("Only Kraken is supported by the Rust core!");
        std::process::exit(1);
    }

    if file_config.dry_run {
        log::warn!("DRY RUN MODE - No real orders will be placed");
    }

    // Create BotConfig
    let mut bot_config = BotConfig::default();
    bot_config.api_key = file_config.exchange.key;
    bot_config.api_secret = file_config.exchange.secret;
    bot_config.max_open_trades = file_config.max_open_trades;

    log::info!(
        "Configuration loaded: {} max trades",
        bot_config.max_open_trades
    );

    // Initialize engine
    let mut engine = TradingEngine::new(bot_config);

    log::info!("Connecting to Kraken...");
    engine.start().await?;

    log::info!(
        "Engine ready: {} EUR pairs, €{:.2} balance",
        engine.all_eur_pairs.len(),
        engine.eur_balance
    );

    // Main trading loop
    log::info!("Starting trading loop (60s intervals)...");
    log::info!("Press Ctrl+C to stop");

    loop {
        match engine.tick().await {
            Ok(_) => {
                log::info!(
                    "Tick complete | Balance: €{:.2} | Open trades: {}",
                    engine.eur_balance,
                    engine.open_trades.len()
                );

                // Log open trades
                for trade in engine.open_trades.values() {
                    let profit = trade.profit_pct(trade.highest_price) * 100.0;
                    log::info!(
                        "  {} | Entry: €{:.6} | P/L: {:.1}% | Time: {}min",
                        trade.pair,
                        trade.entry_price,
                        profit,
                        trade.time_in_trade_min()
                    );
                }
            }
            Err(e) => {
                log::error!("Tick error: {}", e);
            }
        }

        // Wait for next tick
        sleep(Duration::from_secs(60)).await;
    }
}
