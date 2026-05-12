use std::collections::VecDeque;

pub struct PriceBar {
    pub timestamp: u64,
    pub close: f64,
    pub high: f64,
    pub low: f64,
    pub volume: f64,
}

pub struct Indicators {
    bars: VecDeque<PriceBar>,
    max_bars: usize,
}

impl Indicators {
    pub fn new() -> Self {
        Self {
            bars: VecDeque::with_capacity(501),
            max_bars: 500,
        }
    }

    pub fn with_capacity(max_bars: usize) -> Self {
        Self {
            bars: VecDeque::with_capacity(max_bars + 1),
            max_bars,
        }
    }

    pub fn push(&mut self, bar: PriceBar) {
        self.bars.push_back(bar);
        if self.bars.len() > self.max_bars {
            self.bars.pop_front();
        }
    }

    pub fn len(&self) -> usize {
        self.bars.len()
    }

    pub fn last_price(&self) -> Option<f64> {
        self.bars.back().map(|b| b.close)
    }

    pub fn rsi(&self, period: usize) -> Option<f64> {
        self.rsi_at(period, self.bars.len())
    }

    pub fn rsi_prev(&self, period: usize) -> Option<f64> {
        if self.bars.len() < period + 2 {
            return None;
        }
        self.rsi_at(period, self.bars.len() - 1)
    }

    fn rsi_at(&self, period: usize, end: usize) -> Option<f64> {
        if end < period + 1 || end > self.bars.len() {
            return None;
        }

        let p = period as f64;
        let first_end = if end > period * 2 { end - period } else { period + 1 };
        let start = first_end - period;

        let mut avg_gain = 0.0;
        let mut avg_loss = 0.0;
        for i in (start + 1)..=first_end.min(end) {
            let change = self.bars[i].close - self.bars[i - 1].close;
            if change > 0.0 {
                avg_gain += change;
            } else {
                avg_loss += change.abs();
            }
        }
        avg_gain /= p;
        avg_loss /= p;

        for i in (first_end + 1)..end {
            let change = self.bars[i].close - self.bars[i - 1].close;
            let (gain, loss) = if change > 0.0 {
                (change, 0.0)
            } else {
                (0.0, change.abs())
            };
            avg_gain = (avg_gain * (p - 1.0) + gain) / p;
            avg_loss = (avg_loss * (p - 1.0) + loss) / p;
        }

        if avg_loss < f64::EPSILON {
            return Some(100.0);
        }
        let rs = avg_gain / avg_loss;
        Some(100.0 - (100.0 / (1.0 + rs)))
    }

    pub fn ema(&self, period: usize) -> Option<f64> {
        if self.bars.len() < period {
            return None;
        }

        let k = 2.0 / (period as f64 + 1.0);
        let seed: f64 = self.bars.iter().take(period).map(|b| b.close).sum::<f64>() / period as f64;

        let mut ema = seed;
        for bar in self.bars.iter().skip(period) {
            ema = bar.close * k + ema * (1.0 - k);
        }
        Some(ema)
    }

    pub fn atr(&self, period: usize) -> Option<f64> {
        if self.bars.len() < period + 1 {
            return None;
        }

        let start = self.bars.len() - period - 1;
        let mut sum = 0.0;

        for i in (start + 1)..self.bars.len() {
            let bar = &self.bars[i];
            let prev_close = self.bars[i - 1].close;
            let tr = (bar.high - bar.low)
                .max((bar.high - prev_close).abs())
                .max((bar.low - prev_close).abs());
            sum += tr;
        }

        Some(sum / period as f64)
    }

    pub fn atr_pct(&self, period: usize) -> Option<f64> {
        let atr = self.atr(period)?;
        let price = self.last_price()?;
        if price > 0.0 {
            Some(atr / price)
        } else {
            None
        }
    }

    pub fn highest_high(&self, lookback: usize) -> Option<f64> {
        if self.bars.len() < lookback + 1 {
            return None;
        }
        let end = self.bars.len() - 1;
        let start = end - lookback;
        self.bars
            .iter()
            .skip(start)
            .take(lookback)
            .map(|b| b.high)
            .fold(None, |acc: Option<f64>, h| {
                Some(acc.map_or(h, |a: f64| a.max(h)))
            })
    }

    pub fn volume_ratio(&self, lookback: usize) -> Option<f64> {
        if self.bars.len() < lookback + 1 {
            return None;
        }
        let current_vol = self.bars.back()?.volume;
        let end = self.bars.len() - 1;
        let start = end.saturating_sub(lookback);
        let sum: f64 = self.bars.iter().skip(start).take(lookback).map(|b| b.volume).sum();
        let avg = sum / lookback as f64;
        if avg > 0.0 {
            Some(current_vol / avg)
        } else {
            None
        }
    }

    pub fn avg_volume(&self, period: usize) -> Option<f64> {
        if self.bars.len() < period {
            return None;
        }
        let sum: f64 = self.bars.iter().rev().take(period).map(|b| b.volume).sum();
        Some(sum / period as f64)
    }

    pub fn current_volume(&self) -> Option<f64> {
        self.bars.back().map(|b| b.volume)
    }

    pub fn adx(&self, period: usize) -> Option<f64> {
        let needed = period * 2 + 1;
        if self.bars.len() < needed {
            return None;
        }

        let p = period as f64;
        let n = self.bars.len();
        let start = n - needed;

        let mut plus_dm_smooth = 0.0;
        let mut minus_dm_smooth = 0.0;
        let mut tr_smooth = 0.0;

        for i in (start + 1)..=(start + period) {
            let (pdm, mdm, tr) = Self::dm_tr(&self.bars[i], &self.bars[i - 1]);
            plus_dm_smooth += pdm;
            minus_dm_smooth += mdm;
            tr_smooth += tr;
        }

        let mut dx_values = Vec::new();

        if tr_smooth > 0.0 {
            let pdi = plus_dm_smooth / tr_smooth * 100.0;
            let mdi = minus_dm_smooth / tr_smooth * 100.0;
            let di_sum = pdi + mdi;
            if di_sum > 0.0 {
                dx_values.push((pdi - mdi).abs() / di_sum * 100.0);
            }
        }

        for i in (start + period + 1)..n {
            let (pdm, mdm, tr) = Self::dm_tr(&self.bars[i], &self.bars[i - 1]);
            plus_dm_smooth = plus_dm_smooth - plus_dm_smooth / p + pdm;
            minus_dm_smooth = minus_dm_smooth - minus_dm_smooth / p + mdm;
            tr_smooth = tr_smooth - tr_smooth / p + tr;

            if tr_smooth > 0.0 {
                let pdi = plus_dm_smooth / tr_smooth * 100.0;
                let mdi = minus_dm_smooth / tr_smooth * 100.0;
                let di_sum = pdi + mdi;
                if di_sum > 0.0 {
                    dx_values.push((pdi - mdi).abs() / di_sum * 100.0);
                }
            }
        }

        if dx_values.len() < period {
            return None;
        }

        let mut adx = dx_values[..period].iter().sum::<f64>() / p;
        for dx in &dx_values[period..] {
            adx = (adx * (p - 1.0) + dx) / p;
        }

        Some(adx)
    }

    fn dm_tr(bar: &PriceBar, prev: &PriceBar) -> (f64, f64, f64) {
        let high_diff = bar.high - prev.high;
        let low_diff = prev.low - bar.low;

        let plus_dm = if high_diff > low_diff && high_diff > 0.0 {
            high_diff
        } else {
            0.0
        };
        let minus_dm = if low_diff > high_diff && low_diff > 0.0 {
            low_diff
        } else {
            0.0
        };

        let tr = (bar.high - bar.low)
            .max((bar.high - prev.close).abs())
            .max((bar.low - prev.close).abs());

        (plus_dm, minus_dm, tr)
    }

    pub fn momentum_pct(&self, lookback: usize) -> Option<f64> {
        if self.bars.len() < lookback + 1 {
            return None;
        }
        let old = self.bars[self.bars.len() - lookback - 1].close;
        let new = self.bars.back()?.close;
        if old > 0.0 {
            Some((new - old) / old * 100.0)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_indicators(prices: &[f64]) -> Indicators {
        let mut ind = Indicators::with_capacity(500);
        for (i, &p) in prices.iter().enumerate() {
            ind.push(PriceBar {
                timestamp: i as u64,
                close: p,
                high: p,
                low: p,
                volume: 1000.0,
            });
        }
        ind
    }

    #[test]
    fn test_rsi_overbought() {
        let mut prices = vec![100.0];
        for i in 1..=20 {
            prices.push(100.0 + i as f64);
        }
        let ind = make_indicators(&prices);
        let rsi = ind.rsi(14).unwrap();
        assert!(rsi > 70.0, "RSI should be overbought: {}", rsi);
    }

    #[test]
    fn test_rsi_oversold() {
        let mut prices = vec![100.0];
        for i in 1..=20 {
            prices.push(100.0 - i as f64);
        }
        let ind = make_indicators(&prices);
        let rsi = ind.rsi(14).unwrap();
        assert!(rsi < 30.0, "RSI should be oversold: {}", rsi);
    }

    #[test]
    fn test_ema_follows_price() {
        let ind = make_indicators(&[10.0, 10.0, 10.0, 10.0, 10.0, 12.0, 14.0, 16.0, 18.0, 20.0]);
        let ema = ind.ema(10).unwrap();
        assert!(ema > 10.0 && ema < 20.0, "EMA should be between 10 and 20: {}", ema);
    }
}
