use std::fs;
use std::path::Path;
use serde::Deserialize;
use tokio::time::{sleep, Duration};

mod api;
mod config;
mod trading;

use config::BotConfig;
use trading::engine::TradingEngine;

#[derive(Debug, Deserialize)]
struct ExchangeConfig {
    name: String,
    key: String,
    secret: String,
}

#[derive(Debug, Deserialize)]
struct PythonConfig {
    max_open_trades: usize,
    exchange: ExchangeConfig,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    println!("🚀 Starting Matrix Quant Core (Standalone Daemon)");

    // Define the path to the Python config.json
    let config_path = Path::new("/home/meo/trading-bot/config.json");
    
    if !config_path.exists() {
        eprintln!("❌ Config file not found at {:?}", config_path);
        std::process::exit(1);
    }

    let config_str = fs::read_to_string(config_path)?;
    let py_config: PythonConfig = serde_json::from_str(&config_str)?;

    if py_config.exchange.name.to_lowercase() != "kraken" {
        eprintln!("❌ Only Kraken is supported by the Rust core!");
        std::process::exit(1);
    }

    // Map Python config to Rust BotConfig
    let mut config = BotConfig::default();
    config.api_key = py_config.exchange.key;
    config.api_secret = py_config.exchange.secret;
    config.max_open_trades = py_config.max_open_trades;

    println!("✅ Loaded config.json (Kraken API configured for {} slots).", config.max_open_trades);

    let mut engine = TradingEngine::new(config);
    println!("⚙️ Initializing Engine & Connecting to Kraken...");

    // The infinite Daemon Loop
    loop {
        if let Err(e) = engine.run().await {
            eprintln!("⚠️ Engine run error: {:?}", e);
        }
        
        // Polling tick
        sleep(Duration::from_secs(60)).await;
    }
}
