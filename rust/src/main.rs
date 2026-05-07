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

#[derive(Debug, Deserialize)]
struct ExchangeConfig {
    key: String,
    secret: String,
}

#[derive(Debug, Deserialize)]
struct FileConfig {
    #[serde(default = "default_max_trades")]
    max_open_trades: usize,
    exchange: ExchangeConfig,
}

fn default_max_trades() -> usize {
    2
}

const COLLECT_INTERVAL: u64 = 120;
const EXIT_CHECK_INTERVAL: u64 = 30;
const BALANCE_INTERVAL: u64 = 300;
const STATUS_INTERVAL: u64 = 300;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    log::info!("=== Matrix Quant Core v2.0 ===");
    log::info!("Strategy: RSI Mean-Reversion on Liquid Pairs");
    log::info!("Collect: {}s | Exit-check: {}s | Balance: {}s",
        COLLECT_INTERVAL, EXIT_CHECK_INTERVAL, BALANCE_INTERVAL);

    let config_path = find_config()?;
    let config_str = fs::read_to_string(&config_path)?;
    let file_config: FileConfig = serde_json::from_str(&config_str)?;

    let mut bot_config = BotConfig::default();
    bot_config.api_key = file_config.exchange.key;
    bot_config.api_secret = file_config.exchange.secret;
    bot_config.max_open_trades = file_config.max_open_trades;

    log::info!("Config: max_trades={} | RSI oversold={} overbought={} | SL={}x ATR | Trail={}x ATR",
        bot_config.max_open_trades,
        bot_config.rsi_oversold,
        bot_config.rsi_overbought,
        bot_config.hard_sl_atr_mult,
        bot_config.trail_atr_mult,
    );

    let mut engine = TradingEngine::new(bot_config);

    // Retry engine start with backoff (handles no-internet on boot)
    let mut start_attempt = 0u32;
    loop {
        match engine.start().await {
            Ok(_) => break,
            Err(e) => {
                start_attempt += 1;
                let wait = (30 * start_attempt.min(10)) as u64;
                log::error!("Start failed (attempt {}): {} — retrying in {}s", start_attempt, e, wait);
                sleep(Duration::from_secs(wait)).await;
            }
        }
    }

    log::info!("Collecting price data... signals start after ~{} min of data",
        (55 * COLLECT_INTERVAL) / 60);

    let mut last_collect = 0u64;
    let mut last_exit_check = 0u64;
    let mut last_balance = 0u64;
    let mut last_status = 0u64;
    let mut consecutive_errors = 0u32;

    loop {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Back off when network is down
        if consecutive_errors >= 3 {
            let wait = (30 * consecutive_errors.min(20)) as u64;
            log::warn!("Network issues ({} errors) — sleeping {}s", consecutive_errors, wait);
            sleep(Duration::from_secs(wait)).await;
        }

        // Collect tickers (every 2 min)
        if now - last_collect >= COLLECT_INTERVAL {
            last_collect = now;
            engine.tick_count += 1;

            if let Err(e) = engine.collect_tickers().await {
                consecutive_errors += 1;
                log::error!("Ticker collection failed: {}", e);
                continue;
            }
            consecutive_errors = 0;

            engine.reset_daily_if_needed();

            // Scan for entries after collecting
            let signals = engine.scan_entries();
            if !signals.is_empty() {
                log::info!("Entry signals: {}", signals.len());
                for signal in signals.iter().take(2) {
                    log::info!(
                        "  → {} RSI={:.1} ATR={:.2}% Vol={:.1}x score={:.0}",
                        signal.pair, signal.rsi, signal.atr_pct * 100.0,
                        signal.volume_ratio, signal.score
                    );
                }
            }

            for signal in signals.iter().take(1) {
                if engine.open_trades.len() >= engine.config.max_open_trades {
                    break;
                }
                match engine.execute_buy(signal).await {
                    Ok(true) => log::info!("Opened position: {}", signal.pair),
                    Ok(false) => {}
                    Err(e) => {
                        log::error!("Buy error {}: {}", signal.pair, e);
                        if e.to_string().contains("Insufficient funds") {
                            break;
                        }
                    }
                }
            }
        }

        // Check exits (every 30s)
        if now - last_exit_check >= EXIT_CHECK_INTERVAL && !engine.open_trades.is_empty() {
            last_exit_check = now;

            let exits = engine.scan_exits();
            for (pair, reason) in exits {
                if let Err(e) = engine.execute_sell(&pair, &reason).await {
                    log::error!("Sell error {}: {}", pair, e);
                }
            }

            engine.update_trailing_stops().await;
        }

        // Balance update (every 5 min)
        if now - last_balance >= BALANCE_INTERVAL {
            last_balance = now;
            if let Err(e) = engine.fetch_balance().await {
                log::error!("Balance fetch failed: {}", e);
            }
        }

        // Status log (every 5 min)
        if now - last_status >= STATUS_INTERVAL {
            last_status = now;
            engine.log_status();
        }

        sleep(Duration::from_secs(5)).await;
    }
}

fn find_config() -> Result<String> {
    let candidates = [
        "config.json",
        "../config.json",
        "/home/meo/quant-trading-bot/config.json",
    ];
    for path in &candidates {
        if Path::new(path).exists() {
            log::info!("Config: {}", path);
            return Ok(path.to_string());
        }
    }
    anyhow::bail!(
        "config.json not found. Create one with your Kraken API keys:\n\
         {{\n  \"max_open_trades\": 2,\n  \"exchange\": {{\n    \"key\": \"YOUR_KEY\",\n    \"secret\": \"YOUR_SECRET\"\n  }}\n}}"
    );
}
