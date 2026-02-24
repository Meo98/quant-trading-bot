pub mod engine;

#[derive(Debug, Clone)]
pub struct OpenTrade {
    pub pair: String,
    pub entry_price: f64,
    pub amount: f64,
    pub stake_eur: f64,
    pub highest_price: f64,
    pub trailing_stop_pct: f64,
    pub hard_sl_pct: f64,
}

impl OpenTrade {
    pub fn profit_pct(&self, current_price: f64) -> f64 {
        (current_price - self.entry_price) / self.entry_price
    }

    pub fn drawdown_from_high(&self, current_price: f64) -> f64 {
        if self.highest_price == 0.0 {
            return 0.0;
        }
        (current_price - self.highest_price) / self.highest_price
    }
}
