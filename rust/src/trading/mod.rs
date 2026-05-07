pub mod engine;
pub mod indicators;

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenTrade {
    pub pair: String,
    pub kraken_pair: String,
    pub entry_price: f64,
    pub amount: f64,
    pub stake_eur: f64,
    pub highest_price: f64,
    pub entry_time: u64,
    pub stop_loss_order_txid: Option<String>,
    pub server_stop_price: f64,
    /// ATR at time of entry, used for dynamic stop calculation
    pub entry_atr: f64,
    pub exit_reason: Option<String>,
}

impl OpenTrade {
    pub fn profit_pct(&self, current_price: f64) -> f64 {
        if self.entry_price == 0.0 {
            return 0.0;
        }
        (current_price - self.entry_price) / self.entry_price
    }

    pub fn profit_eur(&self, current_price: f64) -> f64 {
        self.amount * current_price - self.stake_eur
    }

    pub fn drawdown_from_high(&self, current_price: f64) -> f64 {
        if self.highest_price == 0.0 {
            return 0.0;
        }
        (current_price - self.highest_price) / self.highest_price
    }

    pub fn age_minutes(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        (now - self.entry_time) / 60
    }
}
