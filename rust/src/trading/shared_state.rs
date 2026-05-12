use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default)]
pub struct IndicatorSnapshot {
    pub last_price: f64,
    pub bid: f64,
    pub ask: f64,
    pub rsi_1m: Option<f64>,
    pub rsi_5m: Option<f64>,
    pub rsi_15m: Option<f64>,
    pub ema_short: Option<f64>,
    pub ema_long: Option<f64>,
    pub atr_pct: Option<f64>,
    pub book_imbalance: Option<f64>,
    pub volume_ratio: Option<f64>,
    pub adx_5m: Option<f64>,
    pub updated_at: u64,
}

pub type SharedIndicators = Arc<RwLock<HashMap<String, IndicatorSnapshot>>>;

pub fn new_shared_indicators() -> SharedIndicators {
    Arc::new(RwLock::new(HashMap::new()))
}
