use crate::trading::indicators::{Indicators, PriceBar};
use crate::trading::orderbook::OrderBookTracker;
use crate::trading::shared_state::SharedIndicators;
use crate::trading::timeframes::CandleAggregator;
use crate::trading::types::*;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

const RSI_PERIOD: usize = 14;
const EMA_SHORT: usize = 9;
const EMA_LONG: usize = 21;
const ATR_PERIOD: usize = 14;
const MIN_CANDLES: usize = 30;
const BREAKOUT_LOOKBACK: usize = 20;
const BREAKOUT_VOL_MULT: f64 = 1.5;
const IMBALANCE_THRESHOLD: f64 = 1.5;
// Tightened from 5.0 → 6.5 on 2026-05-19. Higher conviction = fewer trades but
// better fee:edge ratio. On a small (€350) account where round-trip Kraken fees
// are 0.52% and average winner was €0.06, every fee saved compounds.
const MIN_CONFLUENCE_SCORE: f64 = 6.5;

// Minimum 5-minute ATR (as fraction of price) required to enter. Below this,
// volatility is too low for the trade to overcome fees + spread before timing out.
// 0.30% ATR over 5m ≈ minimum range needed for ~0.5% target trade.
const MIN_ATR_PCT: f64 = 0.003;
const ADX_PERIOD: usize = 14;
const MIN_ADX: f64 = 25.0;


pub struct SignalEngine {
    rx: mpsc::Receiver<MarketEvent>,
    tx: mpsc::Sender<TradeSignal>,
    shared: SharedIndicators,
    aggregators: HashMap<String, CandleAggregator>,
    indicators: HashMap<String, HashMap<Timeframe, Indicators>>,
    books: OrderBookTracker,
    pair_map: HashMap<String, String>,
    cooldowns: HashMap<String, u64>,
}

impl SignalEngine {
    pub fn new(
        rx: mpsc::Receiver<MarketEvent>,
        tx: mpsc::Sender<TradeSignal>,
        shared: SharedIndicators,
        pair_map: HashMap<String, String>,
    ) -> Self {
        Self {
            rx,
            tx,
            shared,
            aggregators: HashMap::new(),
            indicators: HashMap::new(),
            books: OrderBookTracker::new(),
            pair_map,
            cooldowns: HashMap::new(),
        }
    }

    pub fn preload(&mut self, symbol: &str, tf: Timeframe, candles: Vec<Candle>) {
        if candles.is_empty() {
            return;
        }

        let tf_indicators = self
            .indicators
            .entry(symbol.to_string())
            .or_insert_with(HashMap::new);

        let ind = tf_indicators
            .entry(tf)
            .or_insert_with(Indicators::new);

        for candle in &candles {
            ind.push(PriceBar {
                timestamp: candle.timestamp,
                close: candle.close,
                high: candle.high,
                low: candle.low,
                volume: candle.volume,
            });
        }

        let agg = self
            .aggregators
            .entry(symbol.to_string())
            .or_insert_with(CandleAggregator::new);

        if let Some(builder) = agg.builder_mut(&tf) {
            for candle in candles {
                if builder.completed.len() >= 200 {
                    builder.completed.pop_front();
                }
                builder.completed.push_back(candle);
            }
        }
    }

    pub async fn run(&mut self) {
        log::info!("Signal engine started");
        while let Some(event) = self.rx.recv().await {
            match event {
                MarketEvent::Ticker {
                    ref symbol,
                    last,
                    bid,
                    ask,
                    ..
                } => {
                    self.update_shared_price(symbol, last, bid, ask).await;
                }
                MarketEvent::Trade {
                    ref symbol,
                    price,
                    qty,
                    ..
                } => {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();

                    let agg = self
                        .aggregators
                        .entry(symbol.clone())
                        .or_insert_with(CandleAggregator::new);
                    let completed_tfs = agg.on_tick(price, qty, now);

                    if !completed_tfs.is_empty() {
                        self.update_indicators(symbol, &completed_tfs);
                        self.update_shared_indicators(symbol).await;
                        self.check_signals(symbol).await;
                    }
                }
                MarketEvent::BookSnapshot {
                    ref symbol,
                    ref bids,
                    ref asks,
                } => {
                    self.books.apply_snapshot(symbol, bids, asks);
                }
                MarketEvent::BookUpdate {
                    ref symbol,
                    ref bids,
                    ref asks,
                } => {
                    self.books.apply_update(symbol, bids, asks);
                }
                MarketEvent::Disconnected => {
                    log::warn!("Signal engine: WS disconnected");
                }
            }
        }
    }

    fn update_indicators(&mut self, symbol: &str, completed_tfs: &[Timeframe]) {
        let agg = match self.aggregators.get(symbol) {
            Some(a) => a,
            None => return,
        };

        let tf_indicators = self
            .indicators
            .entry(symbol.to_string())
            .or_insert_with(HashMap::new);

        for tf in completed_tfs {
            let builder = match agg.builder(tf) {
                Some(b) => b,
                None => continue,
            };

            let ind = tf_indicators
                .entry(*tf)
                .or_insert_with(Indicators::new);

            if let Some(candle) = builder.completed.back() {
                ind.push(crate::trading::indicators::PriceBar {
                    timestamp: candle.timestamp,
                    close: candle.close,
                    high: candle.high,
                    low: candle.low,
                    volume: candle.volume,
                });
            }
        }
    }

    async fn update_shared_price(&self, symbol: &str, last: f64, bid: f64, ask: f64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mut state = self.shared.write().await;
        let snap = state.entry(symbol.to_string()).or_default();
        snap.last_price = last;
        snap.bid = bid;
        snap.ask = ask;
        snap.updated_at = now;
    }

    async fn update_shared_indicators(&self, symbol: &str) {
        let tf_ind = match self.indicators.get(symbol) {
            Some(i) => i,
            None => return,
        };

        let mut state = self.shared.write().await;
        let snap = state.entry(symbol.to_string()).or_default();

        if let Some(ind) = tf_ind.get(&Timeframe::M1) {
            snap.rsi_1m = ind.rsi(RSI_PERIOD);
            snap.atr_pct = ind.atr_pct(ATR_PERIOD);
        }
        if let Some(ind) = tf_ind.get(&Timeframe::M5) {
            snap.rsi_5m = ind.rsi(RSI_PERIOD);
            snap.ema_short = ind.ema(EMA_SHORT);
            snap.ema_long = ind.ema(EMA_LONG);
        }
        if let Some(ind) = tf_ind.get(&Timeframe::M15) {
            snap.rsi_15m = ind.rsi(RSI_PERIOD);
        }
        snap.book_imbalance = self.books.imbalance(symbol, 10);
        snap.volume_ratio = tf_ind
            .get(&Timeframe::M5)
            .and_then(|i| i.volume_ratio(20));
        snap.adx_5m = tf_ind
            .get(&Timeframe::M5)
            .and_then(|i| i.adx(ADX_PERIOD));
    }

    async fn check_signals(&mut self, symbol: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        if crate::trading::types::STABLECOINS.contains(&symbol) {
            return;
        }

        if let Some(&cooldown_until) = self.cooldowns.get(symbol) {
            if now < cooldown_until {
                return;
            }
        }

        let tf_ind = match self.indicators.get(symbol) {
            Some(i) => i,
            None => return,
        };

        let ind_5m = match tf_ind.get(&Timeframe::M5) {
            Some(i) if i.len() >= MIN_CANDLES => i,
            _ => return,
        };

        let kraken_pair = match self.pair_map.get(symbol) {
            Some(p) => p.clone(),
            None => return,
        };

        let price = match ind_5m.last_price() {
            Some(p) => p,
            None => return,
        };

        // ATR floor: skip pairs where volatility is too low for a viable trade.
        // Below MIN_ATR_PCT, expected move is smaller than fees + spread, so any
        // trade dies by fee-drag before profit.
        if let Some(atr_pct) = ind_5m.atr_pct(ATR_PERIOD) {
            if atr_pct < MIN_ATR_PCT {
                return;
            }
        }

        // ADX regime filter: only trade when there's a clear trend
        let adx = ind_5m.adx(ADX_PERIOD);
        match adx {
            Some(v) if v >= MIN_ADX => {}
            _ => return,
        }
        let adx_val = adx.unwrap();

        let mut score = 0.0f64;
        let mut components = Vec::new();

        // ADX trend strength bonus
        if adx_val > 35.0 {
            score += 0.5;
            components.push(SignalComponent::AdxStrong(adx_val));
        }

        // 1. RSI bounce (5min timeframe)
        if let (Some(rsi), Some(prev_rsi)) = (ind_5m.rsi(RSI_PERIOD), ind_5m.rsi_prev(RSI_PERIOD))
        {
            if rsi > 25.0 && rsi < 40.0 && prev_rsi < 30.0 {
                let strength = 1.0 + (30.0 - prev_rsi) / 30.0;
                score += strength;
                components.push(SignalComponent::RsiBounce(rsi));
            }
        }

        // 2. Momentum breakout (5min)
        if let Some(highest) = ind_5m.highest_high(BREAKOUT_LOOKBACK) {
            if price > highest {
                if let Some(vol_ratio) = ind_5m.volume_ratio(BREAKOUT_LOOKBACK) {
                    if vol_ratio > BREAKOUT_VOL_MULT {
                        score += 1.5;
                        components.push(SignalComponent::MomentumBreakout(vol_ratio));
                    }
                }
            }
        }

        // 3. EMA alignment (5min: short > long = bullish)
        if let (Some(ema_s), Some(ema_l)) = (ind_5m.ema(EMA_SHORT), ind_5m.ema(EMA_LONG)) {
            if ema_s > ema_l && price > ema_s {
                score += 1.0;
                components.push(SignalComponent::EmaAlignment);
            }
        }

        // 4. Book imbalance
        if let Some(imb) = self.books.imbalance(symbol, 10) {
            if imb > IMBALANCE_THRESHOLD {
                score += 1.0;
                components.push(SignalComponent::BookImbalance(imb));
            }
        }

        // 5. Multi-timeframe agreement
        let mut bullish_tfs = 0u8;
        for tf in Timeframe::all() {
            if let Some(ind) = tf_ind.get(tf) {
                if let (Some(rsi), Some(ema_s), Some(ema_l)) =
                    (ind.rsi(RSI_PERIOD), ind.ema(EMA_SHORT), ind.ema(EMA_LONG))
                {
                    if rsi > 40.0 && rsi < 70.0 && ema_s > ema_l {
                        bullish_tfs += 1;
                    }
                }
            }
        }
        if bullish_tfs >= 2 {
            score += 1.0;
            components.push(SignalComponent::MultiTfAgreement(bullish_tfs));
        }

        // 6. Volume surge
        if let Some(vol_ratio) = ind_5m.volume_ratio(20) {
            if vol_ratio > 2.0 {
                let bonus = ((vol_ratio - 2.0) / 2.0).min(1.0);
                score += 0.5 + bonus * 0.5;
                components.push(SignalComponent::VolumeSurge(vol_ratio));
            }
        }

        // 7. Spread check
        if let Some(spread) = self.books.spread_pct(symbol) {
            if spread < 0.3 {
                score += 0.5;
                components.push(SignalComponent::SpreadTight(spread));
            }
        }

        if score >= MIN_CONFLUENCE_SCORE {
            let atr_pct = ind_5m.atr_pct(ATR_PERIOD).unwrap_or(0.02);

            log::info!(
                "SIGNAL {} | score={:.1} | components: {:?}",
                symbol,
                score,
                components
            );

            let signal = TradeSignal {
                pair: symbol.to_string(),
                kraken_pair,
                price,
                strength: score,
                atr_pct,
                components,
            };

            let _ = self.tx.send(signal).await;
            self.cooldowns.insert(symbol.to_string(), now + 3600);
        }
    }
}
