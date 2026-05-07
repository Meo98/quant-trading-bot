use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    pub api_key: String,
    pub api_secret: String,

    /// Max concurrent open positions (default: 2)
    pub max_open_trades: usize,
    /// Max pairs to watch from liquidity ranking (default: 30)
    pub max_watched_pairs: usize,
    /// Minimum 24h EUR volume to consider a pair (default: 50_000)
    pub min_volume_eur: f64,

    /// RSI threshold for oversold signal (default: 30)
    pub rsi_oversold: f64,
    /// RSI threshold for overbought take-profit (default: 72)
    pub rsi_overbought: f64,

    /// Hard stop = entry - (ATR * this multiplier) (default: 2.5)
    pub hard_sl_atr_mult: f64,
    /// Trailing stop = peak - (ATR * this multiplier) (default: 2.0)
    pub trail_atr_mult: f64,

    /// Max hold time in minutes before time-stop (default: 4320 = 72h)
    pub max_hold_minutes: u64,
    /// Max daily drawdown as fraction of start balance (default: 0.15)
    pub max_daily_drawdown: f64,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_secret: String::new(),
            max_open_trades: 2,
            max_watched_pairs: 30,
            min_volume_eur: 50_000.0,
            rsi_oversold: 30.0,
            rsi_overbought: 72.0,
            hard_sl_atr_mult: 2.5,
            trail_atr_mult: 2.0,
            max_hold_minutes: 4320,
            max_daily_drawdown: 0.15,
        }
    }
}
