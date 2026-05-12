use crate::trading::types::{Candle, Timeframe};
use std::collections::{HashMap, VecDeque};

pub struct CandleBuilder {
    interval_secs: u64,
    current: Option<Candle>,
    pub completed: VecDeque<Candle>,
    max_candles: usize,
}

impl CandleBuilder {
    pub fn new(interval_secs: u64, max_candles: usize) -> Self {
        Self {
            interval_secs,
            current: None,
            completed: VecDeque::with_capacity(max_candles),
            max_candles,
        }
    }

    /// Feed a tick. Returns true if a candle was completed.
    pub fn on_tick(&mut self, price: f64, volume: f64, timestamp: u64) -> bool {
        let candle_start = timestamp - (timestamp % self.interval_secs);

        if let Some(ref mut c) = self.current {
            if candle_start > c.timestamp {
                let finished = c.clone();
                if self.completed.len() >= self.max_candles {
                    self.completed.pop_front();
                }
                self.completed.push_back(finished);

                self.current = Some(Candle {
                    open: price,
                    high: price,
                    low: price,
                    close: price,
                    volume,
                    timestamp: candle_start,
                });
                return true;
            }
            c.high = c.high.max(price);
            c.low = c.low.min(price);
            c.close = price;
            c.volume += volume;
        } else {
            self.current = Some(Candle {
                open: price,
                high: price,
                low: price,
                close: price,
                volume,
                timestamp: candle_start,
            });
        }
        false
    }

    pub fn candle_count(&self) -> usize {
        self.completed.len()
    }
}

pub struct CandleAggregator {
    builders: HashMap<Timeframe, CandleBuilder>,
}

impl CandleAggregator {
    pub fn new() -> Self {
        let mut builders = HashMap::new();
        for tf in Timeframe::all() {
            builders.insert(*tf, CandleBuilder::new(tf.secs(), 200));
        }
        Self { builders }
    }

    /// Feed a tick to all timeframes. Returns list of timeframes that completed a candle.
    pub fn on_tick(&mut self, price: f64, volume: f64, timestamp: u64) -> Vec<Timeframe> {
        let mut completed = Vec::new();
        for (tf, builder) in &mut self.builders {
            if builder.on_tick(price, volume, timestamp) {
                completed.push(*tf);
            }
        }
        completed
    }

    pub fn builder(&self, tf: &Timeframe) -> Option<&CandleBuilder> {
        self.builders.get(tf)
    }

    pub fn builder_mut(&mut self, tf: &Timeframe) -> Option<&mut CandleBuilder> {
        self.builders.get_mut(tf)
    }
}
