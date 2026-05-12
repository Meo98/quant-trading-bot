use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

mod api;
mod config;
mod trading;

use api::ws_client::KrakenWsClient;
use config::BotConfig;
use trading::engine::TradingEngine;
use trading::shared_state::new_shared_indicators;
use trading::signal::SignalEngine;
use trading::types::{Timeframe, TradeSignal};

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

const EXIT_CHECK_INTERVAL: u64 = 10;
const BALANCE_INTERVAL: u64 = 60;
const STATUS_INTERVAL: u64 = 300;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    // Single instance guard
    let pid_path = "/tmp/matrix_quant.pid";
    if Path::new(pid_path).exists() {
        if let Ok(old_pid) = fs::read_to_string(pid_path) {
            let old_pid = old_pid.trim();
            if Path::new(&format!("/proc/{}", old_pid)).exists() {
                log::error!("Another instance is already running (PID {}). Exiting.", old_pid);
                process::exit(1);
            }
        }
    }
    fs::write(pid_path, process::id().to_string()).ok();

    log::info!("=== Matrix Quant Core v3.1 ===");
    log::info!("Strategy: Chandelier Trailing + ADX Filter + Confluence ≥4.0");
    log::info!("Exit-check: {}s | Balance: {}s | Status: {}s",
        EXIT_CHECK_INTERVAL, BALANCE_INTERVAL, STATUS_INTERVAL);

    let config_path = find_config()?;
    let config_str = fs::read_to_string(&config_path)?;
    let file_config: FileConfig = serde_json::from_str(&config_str)?;

    let mut bot_config = BotConfig::default();
    bot_config.api_key = file_config.exchange.key;
    bot_config.api_secret = file_config.exchange.secret;
    bot_config.max_open_trades = file_config.max_open_trades;

    log::info!("Config: max_trades={} | SL={}x ATR | Trail={}x ATR",
        bot_config.max_open_trades,
        bot_config.hard_sl_atr_mult,
        bot_config.trail_atr_mult,
    );

    let shared = new_shared_indicators();

    let mut engine = TradingEngine::new(bot_config);
    engine.set_shared(shared.clone());

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

    let ws_symbols: Vec<String> = engine.all_eur_pairs.keys().cloned().collect();
    let pair_map: HashMap<String, String> = engine.all_eur_pairs.clone();

    log::info!("Subscribing to {} pairs via WebSocket", ws_symbols.len());

    let (market_tx, market_rx) = mpsc::channel(10000);
    let (signal_tx, mut signal_rx) = mpsc::channel::<TradeSignal>(100);

    let mut signal_engine = SignalEngine::new(market_rx, signal_tx, shared.clone(), pair_map);

    // Preload historical OHLC data so signals work immediately
    log::info!("Fetching tickers for pair selection...");
    let preload_pairs: Vec<(String, String)> = match engine.collect_tickers().await {
        Ok(_) => engine
            .liquid_pairs
            .iter()
            .take(50)
            .map(|(ws, kr, _)| (ws.clone(), kr.clone()))
            .collect(),
        Err(e) => {
            log::warn!("Ticker fetch failed ({}), using alphabetical fallback", e);
            engine.top_liquid_pairs(50)
        }
    };

    log::info!("Preloading OHLC for {} pairs (M1/M5/M15)...", preload_pairs.len());
    let timeframes = [(1u64, Timeframe::M1), (5, Timeframe::M5), (15, Timeframe::M15)];
    let mut loaded = 0u32;
    let mut errors = 0u32;
    for (ws_name, kraken_pair) in &preload_pairs {
        for (interval, tf) in &timeframes {
            match engine.fetch_ohlc(kraken_pair, *interval).await {
                Ok(candles) if !candles.is_empty() => {
                    signal_engine.preload(ws_name, *tf, candles);
                    loaded += 1;
                }
                Ok(_) => {}
                Err(e) => {
                    errors += 1;
                    if errors <= 3 {
                        log::warn!("OHLC fetch {} {}m: {}", ws_name, interval, e);
                    }
                }
            }
            sleep(Duration::from_secs(1)).await;
        }
        if loaded % 30 == 0 && loaded > 0 {
            log::info!("  preloaded {}/{} datasets...", loaded, preload_pairs.len() * 3);
        }
    }
    log::info!("Preload complete: {} datasets loaded ({} errors)", loaded, errors);

    let ws_client = KrakenWsClient::new(ws_symbols, market_tx);
    tokio::spawn(async move { ws_client.run().await });
    tokio::spawn(async move { signal_engine.run().await });

    log::info!("All tasks spawned — waiting for signals...");

    let mut last_exit_check = 0u64;
    let mut last_balance = 0u64;
    let mut last_status = 0u64;

    loop {
        tokio::select! {
            signal = signal_rx.recv() => {
                match signal {
                    Some(sig) => {
                        log::info!("Received signal: {} | strength={:.1} | {:?}",
                            sig.pair, sig.strength, sig.components);
                        match engine.execute_buy_from_signal(&sig).await {
                            Ok(true) => log::info!("Opened position: {}", sig.pair),
                            Ok(false) => {}
                            Err(e) => {
                                log::error!("Buy error {}: {}", sig.pair, e);
                                if e.to_string().contains("Insufficient funds") {
                                    engine.eur_balance = 0.0;
                                }
                            }
                        }
                    }
                    None => {
                        log::error!("Signal channel closed — restarting");
                        break;
                    }
                }
            }
            _ = sleep(Duration::from_secs(2)) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                engine.update_peaks().await;

                if now - last_exit_check >= EXIT_CHECK_INTERVAL && !engine.open_trades.is_empty() {
                    last_exit_check = now;

                    let (prices, rsis) = engine.snapshot_prices_rsis().await;
                    let exits = engine.scan_exits_ws(&prices, &rsis).await;
                    for (pair, reason) in exits {
                        if let Err(e) = engine.execute_sell(&pair, &reason).await {
                            log::error!("Sell error {}: {}", pair, e);
                        }
                    }
                    engine.update_trailing_stops_ws(&prices, &rsis).await;
                }

                if now - last_balance >= BALANCE_INTERVAL {
                    last_balance = now;
                    if let Err(e) = engine.fetch_balance().await {
                        log::error!("Balance fetch failed: {}", e);
                    }
                    engine.reset_daily_if_needed();
                }

                if now - last_status >= STATUS_INTERVAL {
                    last_status = now;
                    engine.tick_count += 1;
                    engine.log_status_ws().await;
                }
            }
        }
    }

    Ok(())
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
