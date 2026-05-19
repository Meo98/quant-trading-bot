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

    /// Max hold time in minutes before time-stop (default: 1440 = 24h)
    pub max_hold_minutes: u64,
    /// Max daily drawdown as fraction of start balance (default: 0.15)
    pub max_daily_drawdown: f64,
    /// Max trades to open per day. Prevents overtrading on high-noise days
    /// that would churn through fees. 0 = unlimited.
    pub max_trades_per_day: u32,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_secret: String::new(),
            max_open_trades: 3,
            max_watched_pairs: 30,
            min_volume_eur: 50_000.0,
            rsi_oversold: 30.0,
            rsi_overbought: 80.0,
            hard_sl_atr_mult: 2.0,
            trail_atr_mult: 1.0,
            // Bumped 120 → 360 on 2026-05-19. The 2h TIME-STOP was killing
            // slightly-losing trades that would have recovered. New tier-based
            // TIME-STOP in engine.rs differentiates between clear losers
            // (-0.5 ATR or worse) at 6h vs anything stagnant at 12h.
            max_hold_minutes: 360,
            max_daily_drawdown: 0.05,
            // Allow up to 5 new trades per day. With max_open_trades=3 and
            // 2-12h holds, 5/day is reasonable selectivity.
            max_trades_per_day: 5,
        }
    }
}
