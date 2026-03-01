//! Flutter Bridge API
//!
//! This module provides the interface between Flutter and the Rust trading engine.
//! All functions here are exposed to Dart via flutter_rust_bridge.

use crate::config::BotConfig;
use crate::trading::engine::TradingEngine;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Global trading engine instance (thread-safe)
static ENGINE: Lazy<Mutex<Option<TradingEngine>>> = Lazy::new(|| Mutex::new(None));

/// Engine status for Flutter UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatusDto {
    pub is_running: bool,
    pub eur_balance: f64,
    pub open_trades_count: i32,
    pub total_pairs: i32,
    pub last_tick: i64,
    pub error_message: String,
}

/// Configuration DTO for Flutter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDto {
    pub api_key: String,
    pub api_secret: String,
    pub max_open_trades: i32,
    pub min_pct_24h: f64,
    pub min_pct_15m: f64,
    pub min_pct_1h: f64,
    pub min_volume_eur: f64,
    pub trailing_stop_pct: f64,
    pub hard_sl_pct: f64,
}

/// Trade info DTO for Flutter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeDto {
    pub pair: String,
    pub entry_price: f64,
    pub current_price: f64,
    pub amount: f64,
    pub stake_eur: f64,
    pub profit_pct: f64,
    pub profit_eur: f64,
    pub time_in_trade_min: i64,
    pub trailing_stop_pct: f64,
}

// ============================================================================
// Flutter Bridge Functions
// ============================================================================

/// Initialize the Flutter-Rust bridge
#[flutter_rust_bridge::frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();

    // Initialize logging for mobile
    #[cfg(target_os = "android")]
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("MatrixQuant"),
    );
}

/// Simple greeting function (for testing bridge connection)
#[flutter_rust_bridge::frb(sync)]
pub fn greet(name: String) -> String {
    format!("Hello from Rust, {}!", name)
}

/// Initialize the trading engine with configuration
#[flutter_rust_bridge::frb]
pub async fn initialize_engine(config: ConfigDto) -> Result<String, String> {
    let bot_config = BotConfig {
        api_key: config.api_key,
        api_secret: config.api_secret,
        max_open_trades: config.max_open_trades as usize,
        min_pct_24h: config.min_pct_24h,
        min_pct_15m: config.min_pct_15m,
        min_pct_1h: config.min_pct_1h,
        min_volume_eur: config.min_volume_eur,
        trailing_stop_pct: config.trailing_stop_pct / 100.0, // Convert from percentage
        hard_sl_pct: config.hard_sl_pct / 100.0,
        ..Default::default()
    };

    let mut engine = TradingEngine::new(bot_config);

    match engine.start().await {
        Ok(_) => {
            let msg = format!(
                "Engine initialized: {} pairs, €{:.2} balance",
                engine.all_eur_pairs.len(),
                engine.eur_balance
            );

            let mut guard = ENGINE.lock().map_err(|e| e.to_string())?;
            *guard = Some(engine);

            Ok(msg)
        }
        Err(e) => Err(format!("Failed to start engine: {}", e)),
    }
}

/// Start the trading engine (must be initialized first)
#[flutter_rust_bridge::frb]
pub async fn start_engine() -> Result<String, String> {
    let mut guard = ENGINE.lock().map_err(|e| e.to_string())?;

    match guard.as_mut() {
        Some(engine) => {
            engine.is_running = true;
            Ok("Engine started".to_string())
        }
        None => Err("Engine not initialized. Call initialize_engine first.".to_string()),
    }
}

/// Stop the trading engine
#[flutter_rust_bridge::frb(sync)]
pub fn stop_engine() -> Result<String, String> {
    let mut guard = ENGINE.lock().map_err(|e| e.to_string())?;

    match guard.as_mut() {
        Some(engine) => {
            engine.stop();
            Ok("Engine stopped".to_string())
        }
        None => Err("Engine not initialized".to_string()),
    }
}

/// Run a single trading tick (called by background service)
#[flutter_rust_bridge::frb]
pub async fn run_tick() -> Result<String, String> {
    let engine_opt = {
        let guard = ENGINE.lock().map_err(|e| e.to_string())?;
        guard.clone()
    };

    match engine_opt {
        Some(mut engine) => {
            if !engine.is_running {
                return Ok("Engine is stopped".to_string());
            }

            match engine.tick().await {
                Ok(_) => {
                    // Update the global engine state
                    let mut guard = ENGINE.lock().map_err(|e| e.to_string())?;
                    *guard = Some(engine.clone());

                    Ok(format!(
                        "Tick complete: {} open trades, €{:.2} balance",
                        engine.open_trades.len(),
                        engine.eur_balance
                    ))
                }
                Err(e) => Err(format!("Tick error: {}", e)),
            }
        }
        None => Err("Engine not initialized".to_string()),
    }
}

/// Get current engine status
#[flutter_rust_bridge::frb(sync)]
pub fn get_status() -> Result<EngineStatusDto, String> {
    let guard = ENGINE.lock().map_err(|e| e.to_string())?;

    match guard.as_ref() {
        Some(engine) => {
            let status = engine.get_status();
            Ok(EngineStatusDto {
                is_running: status.is_running,
                eur_balance: status.eur_balance,
                open_trades_count: status.open_trades_count as i32,
                total_pairs: status.total_pairs_scanned as i32,
                last_tick: status.last_tick as i64,
                error_message: status.error_message.unwrap_or_default(),
            })
        }
        None => Ok(EngineStatusDto {
            is_running: false,
            eur_balance: 0.0,
            open_trades_count: 0,
            total_pairs: 0,
            last_tick: 0,
            error_message: "Engine not initialized".to_string(),
        }),
    }
}

/// Get list of open trades
#[flutter_rust_bridge::frb(sync)]
pub fn get_open_trades() -> Result<Vec<TradeDto>, String> {
    let guard = ENGINE.lock().map_err(|e| e.to_string())?;

    match guard.as_ref() {
        Some(engine) => {
            let trades: Vec<TradeDto> = engine
                .get_open_trades()
                .iter()
                .map(|t| TradeDto {
                    pair: t.pair.clone(),
                    entry_price: t.entry_price,
                    current_price: t.highest_price, // Best estimate without API call
                    amount: t.amount,
                    stake_eur: t.stake_eur,
                    profit_pct: t.profit_pct(t.highest_price) * 100.0,
                    profit_eur: t.profit_eur(t.highest_price),
                    time_in_trade_min: t.time_in_trade_min() as i64,
                    trailing_stop_pct: t.trailing_stop_pct * 100.0,
                })
                .collect();
            Ok(trades)
        }
        None => Ok(vec![]),
    }
}

/// Check if engine is initialized
#[flutter_rust_bridge::frb(sync)]
pub fn is_initialized() -> bool {
    ENGINE.lock().map(|g| g.is_some()).unwrap_or(false)
}

/// Update engine configuration (requires restart)
#[flutter_rust_bridge::frb(sync)]
pub fn update_config(config: ConfigDto) -> Result<String, String> {
    let mut guard = ENGINE.lock().map_err(|e| e.to_string())?;

    match guard.as_mut() {
        Some(engine) => {
            engine.config.max_open_trades = config.max_open_trades as usize;
            engine.config.min_pct_24h = config.min_pct_24h;
            engine.config.min_pct_15m = config.min_pct_15m;
            engine.config.min_pct_1h = config.min_pct_1h;
            engine.config.min_volume_eur = config.min_volume_eur;
            engine.config.trailing_stop_pct = config.trailing_stop_pct / 100.0;
            engine.config.hard_sl_pct = config.hard_sl_pct / 100.0;
            Ok("Configuration updated".to_string())
        }
        None => Err("Engine not initialized".to_string()),
    }
}

/// Get current configuration
#[flutter_rust_bridge::frb(sync)]
pub fn get_config() -> Result<ConfigDto, String> {
    let guard = ENGINE.lock().map_err(|e| e.to_string())?;

    match guard.as_ref() {
        Some(engine) => Ok(ConfigDto {
            api_key: "***".to_string(), // Never expose keys
            api_secret: "***".to_string(),
            max_open_trades: engine.config.max_open_trades as i32,
            min_pct_24h: engine.config.min_pct_24h,
            min_pct_15m: engine.config.min_pct_15m,
            min_pct_1h: engine.config.min_pct_1h,
            min_volume_eur: engine.config.min_volume_eur,
            trailing_stop_pct: engine.config.trailing_stop_pct * 100.0,
            hard_sl_pct: engine.config.hard_sl_pct * 100.0,
        }),
        None => Err("Engine not initialized".to_string()),
    }
}
