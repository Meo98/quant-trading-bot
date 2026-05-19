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
    /// Trailing stop = peak - (ATR * this multiplier) (default: 2.0).
    /// This is the BASE — engine.rs may tighten via RSI extremes but never below
    /// `trail_atr_min`. Le Beau's original Chandelier uses 3.0; literature for
    /// short-TF crypto recommends 1.5 as minimum floor.
    pub trail_atr_mult: f64,
    /// Minimum trail multiplier — never tighten trail below this. Prevents the
    /// 0.5× ATR death-trap (see 2026-05-19 research review: any noise candle
    /// stops you out at 0.5× ATR on 5m crypto, leading to €0.06 avg winners
    /// vs. potential 0.5-2% per trade with 1.5× floor).
    pub trail_atr_min: f64,

    /// Max hold time in minutes before time-stop (default: 1440 = 24h)
    pub max_hold_minutes: u64,
    /// Max daily drawdown as fraction of start balance (default: 0.15)
    pub max_daily_drawdown: f64,
    /// Max trades to open per day. Prevents overtrading on high-noise days
    /// that would churn through fees. 0 = unlimited.
    pub max_trades_per_day: u32,
    /// If Some(€), use this fixed EUR stake per trade instead of %-of-equity.
    /// Recommended for accounts < €1000 where %-sizing is meaningless due to
    /// MIN_STAKE constraints (Carver, Tharp et al). With None, falls back to
    /// the BASE_RISK_PCT / MAX_RISK_PCT signal-strength scaling.
    pub fixed_stake_eur: Option<f64>,
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
            // 1.0 → 3.0 on 2026-05-19. Le Beau's original Chandelier default.
            // Combined with trail_atr_min=1.5 floor, lets winners actually run
            // instead of being knocked out by single noise candles.
            trail_atr_mult: 3.0,
            trail_atr_min: 1.5,
            // Bumped 360 → 720 (12h) on 2026-05-19. Longer holds let momentum
            // unfold; fee/move ratio drops from ~50% (1h hold) to ~10% (12h).
            max_hold_minutes: 720,
            max_daily_drawdown: 0.05,
            // Reduced 5 → 3 on 2026-05-19. With higher selectivity (regime-
            // aware confluence + 0.5% ATR floor), expect even fewer high-quality
            // signals. Hard cap prevents overtrading on noisy days.
            max_trades_per_day: 3,
            // Fixed €10 stake for small accounts. Kelly-criterion is negative
            // at PF<1, so %-scaling is theatre. Switch to None and adjust
            // BASE_RISK_PCT in engine.rs once account exceeds ~€1000.
            fixed_stake_eur: Some(10.0),
        }
    }
}
