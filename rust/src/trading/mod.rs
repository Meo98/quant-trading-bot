//! Trading module - Core trading logic and data structures
//!
//! This module contains:
//! - `OpenTrade` - Represents an active position
//! - `TradingEngine` - Main trading loop and strategy logic

pub mod engine;

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents an open/active trading position
#[derive(Debug, Clone, Serialize, Deserialize)]
#[flutter_rust_bridge::frb(ignore)]
pub struct OpenTrade {
    /// Display name (e.g., "BTC/EUR")
    pub pair: String,
    /// Kraken API pair name (e.g., "XXBTZEUR")
    pub kraken_pair: String,
    /// Entry price in EUR
    pub entry_price: f64,
    /// Amount of the asset held
    pub amount: f64,
    /// EUR value at entry
    pub stake_eur: f64,
    /// Highest price seen since entry (for trailing stop)
    pub highest_price: f64,
    /// Current trailing stop percentage (dynamic based on volatility)
    pub trailing_stop_pct: f64,
    /// Hard stop-loss percentage (emergency exit)
    pub hard_sl_pct: f64,
    /// Unix timestamp when trade was entered
    pub entry_time: u64,
}

impl OpenTrade {
    /// Calculate current profit/loss as a percentage
    ///
    /// Returns positive for profit, negative for loss
    pub fn profit_pct(&self, current_price: f64) -> f64 {
        if self.entry_price == 0.0 {
            return 0.0;
        }
        (current_price - self.entry_price) / self.entry_price
    }

    /// Calculate current profit/loss in EUR
    pub fn profit_eur(&self, current_price: f64) -> f64 {
        let current_value = self.amount * current_price;
        current_value - self.stake_eur
    }

    /// Calculate drawdown from highest price as a percentage
    ///
    /// Returns negative value (e.g., -0.10 for 10% drawdown)
    pub fn drawdown_from_high(&self, current_price: f64) -> f64 {
        if self.highest_price == 0.0 {
            return 0.0;
        }
        (current_price - self.highest_price) / self.highest_price
    }

    /// Calculate time in trade in minutes
    pub fn time_in_trade_min(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        (now - self.entry_time) / 60
    }

    /// Get current value in EUR
    pub fn current_value(&self, current_price: f64) -> f64 {
        self.amount * current_price
    }
}

/// Trade information for Flutter UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
#[flutter_rust_bridge::frb(ignore)]
pub struct TradeInfo {
    pub pair: String,
    pub entry_price: f64,
    pub current_price: f64,
    pub amount: f64,
    pub stake_eur: f64,
    pub profit_pct: f64,
    pub profit_eur: f64,
    pub highest_price: f64,
    pub trailing_stop_pct: f64,
    pub time_in_trade_min: u64,
}

impl OpenTrade {
    /// Convert to TradeInfo for UI display
    pub fn to_info(&self, current_price: f64) -> TradeInfo {
        TradeInfo {
            pair: self.pair.clone(),
            entry_price: self.entry_price,
            current_price,
            amount: self.amount,
            stake_eur: self.stake_eur,
            profit_pct: self.profit_pct(current_price) * 100.0,
            profit_eur: self.profit_eur(current_price),
            highest_price: self.highest_price,
            trailing_stop_pct: self.trailing_stop_pct * 100.0,
            time_in_trade_min: self.time_in_trade_min(),
        }
    }
}
