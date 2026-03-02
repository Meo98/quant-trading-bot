use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[flutter_rust_bridge::frb(ignore)]
pub struct BotConfig {
    pub api_key: String,
    pub api_secret: String,
    
    // Pump Logic
    pub min_price: f64,
    pub min_pct_24h: f64,
    pub min_pct_15m: f64,
    pub min_pct_1h: f64,
    pub min_volume_eur: f64,
    
    // Trailing Stop Logic
    pub trailing_stop_pct: f64,
    pub hard_sl_pct: f64,
    pub step_up_1_profit: f64,
    pub step_up_1_trailing: f64,
    pub step_up_2_profit: f64,
    pub step_up_2_trailing: f64,
    
    // Allocation
    pub max_open_trades: usize,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_secret: String::new(),
            min_price: 0.00000001,
            min_pct_24h: 5.0,
            min_pct_15m: 1.0,
            min_pct_1h: 2.0,
            min_volume_eur: 10000.0,
            trailing_stop_pct: 0.10,
            hard_sl_pct: -0.15,
            step_up_1_profit: 0.20,
            step_up_1_trailing: 0.15,
            step_up_2_profit: 0.50,
            step_up_2_trailing: 0.25,
            max_open_trades: 3,
        }
    }
}
