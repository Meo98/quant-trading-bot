use std::collections::VecDeque;

pub struct PriceBar {
    pub timestamp: u64,
    pub price: f64,
    pub volume_eur: f64,
}

pub struct Indicators {
    bars: VecDeque<PriceBar>,
    max_bars: usize,
}

impl Indicators {
    pub fn new(max_bars: usize) -> Self {
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
        self.bars.back().map(|b| b.price)
    }

    /// RSI using exponential moving average of gains/losses (Wilder's method)
    pub fn rsi(&self, period: usize) -> Option<f64> {
        if self.bars.len() < period + 1 {
            return None;
        }

        let prices: Vec<f64> = self.bars.iter().map(|b| b.price).collect();
        let start = prices.len() - period - 1;
        let slice = &prices[start..];

        let mut avg_gain = 0.0;
        let mut avg_loss = 0.0;

        for i in 1..=period {
            let change = slice[i] - slice[i - 1];
            if change > 0.0 {
                avg_gain += change;
            } else {
                avg_loss += change.abs();
            }
        }
        avg_gain /= period as f64;
        avg_loss /= period as f64;

        if avg_loss == 0.0 {
            return Some(100.0);
        }

        let rs = avg_gain / avg_loss;
        Some(100.0 - (100.0 / (1.0 + rs)))
    }

    /// RSI one bar ago (for crossover detection)
    pub fn rsi_prev(&self, period: usize) -> Option<f64> {
        if self.bars.len() < period + 2 {
            return None;
        }

        let prices: Vec<f64> = self.bars.iter().map(|b| b.price).collect();
        let end = prices.len() - 1;
        let start = end - period - 1;
        let slice = &prices[start..end];

        let mut avg_gain = 0.0;
        let mut avg_loss = 0.0;

        for i in 1..=period {
            let change = slice[i] - slice[i - 1];
            if change > 0.0 {
                avg_gain += change;
            } else {
                avg_loss += change.abs();
            }
        }
        avg_gain /= period as f64;
        avg_loss /= period as f64;

        if avg_loss == 0.0 {
            return Some(100.0);
        }

        let rs = avg_gain / avg_loss;
        Some(100.0 - (100.0 / (1.0 + rs)))
    }

    /// Exponential Moving Average
    pub fn ema(&self, period: usize) -> Option<f64> {
        if self.bars.len() < period {
            return None;
        }

        let prices: Vec<f64> = self.bars.iter().map(|b| b.price).collect();
        let start = prices.len() - period;
        let k = 2.0 / (period as f64 + 1.0);

        let mut ema = prices[start];
        for &p in &prices[start + 1..] {
            ema = p * k + ema * (1.0 - k);
        }
        Some(ema)
    }

    /// Average True Range approximated from price changes
    pub fn atr(&self, period: usize) -> Option<f64> {
        if self.bars.len() < period + 1 {
            return None;
        }

        let prices: Vec<f64> = self.bars.iter().map(|b| b.price).collect();
        let start = prices.len() - period - 1;
        let mut sum = 0.0;

        for i in (start + 1)..prices.len() {
            sum += (prices[i] - prices[i - 1]).abs();
        }

        Some(sum / period as f64)
    }

    /// ATR as percentage of current price
    pub fn atr_pct(&self, period: usize) -> Option<f64> {
        let atr = self.atr(period)?;
        let price = self.last_price()?;
        if price > 0.0 {
            Some(atr / price)
        } else {
            None
        }
    }

    /// Average volume over last N bars
    pub fn avg_volume(&self, period: usize) -> Option<f64> {
        if self.bars.len() < period {
            return None;
        }
        let sum: f64 = self.bars.iter().rev().take(period).map(|b| b.volume_eur).sum();
        Some(sum / period as f64)
    }

    /// Current bar's volume
    pub fn current_volume(&self) -> Option<f64> {
        self.bars.back().map(|b| b.volume_eur)
    }

    /// Price change over last N bars as percentage
    pub fn momentum_pct(&self, lookback: usize) -> Option<f64> {
        if self.bars.len() < lookback + 1 {
            return None;
        }
        let old = self.bars[self.bars.len() - lookback - 1].price;
        let new = self.bars.back()?.price;
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
        let mut ind = Indicators::new(500);
        for (i, &p) in prices.iter().enumerate() {
            ind.push(PriceBar {
                timestamp: i as u64,
                price: p,
                volume_eur: 1000.0,
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
