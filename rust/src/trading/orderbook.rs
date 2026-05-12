use crate::trading::types::Level;
use std::collections::HashMap;

pub struct OrderBook {
    bids: Vec<Level>,
    asks: Vec<Level>,
    max_depth: usize,
}

impl OrderBook {
    fn new(max_depth: usize) -> Self {
        Self {
            bids: Vec::new(),
            asks: Vec::new(),
            max_depth,
        }
    }

    pub fn apply_snapshot(&mut self, bids: &[Level], asks: &[Level]) {
        self.bids = bids.iter().take(self.max_depth).copied().collect();
        self.asks = asks.iter().take(self.max_depth).copied().collect();
        self.bids.sort_by(|a, b| b.price.partial_cmp(&a.price).unwrap());
        self.asks.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());
    }

    pub fn apply_update(&mut self, bids: &[Level], asks: &[Level]) {
        for level in bids {
            Self::upsert_sorted(&mut self.bids, *level, true);
        }
        for level in asks {
            Self::upsert_sorted(&mut self.asks, *level, false);
        }
        self.bids.truncate(self.max_depth);
        self.asks.truncate(self.max_depth);
    }

    fn upsert_sorted(levels: &mut Vec<Level>, new: Level, descending: bool) {
        levels.retain(|l| (l.price - new.price).abs() > f64::EPSILON * 1000.0);
        if new.qty > 0.0 {
            let pos = if descending {
                levels.partition_point(|l| l.price > new.price)
            } else {
                levels.partition_point(|l| l.price < new.price)
            };
            levels.insert(pos, new);
        }
    }

    pub fn imbalance(&self, levels: usize) -> f64 {
        let bid_vol: f64 = self.bids.iter().take(levels).map(|l| l.price * l.qty).sum();
        let ask_vol: f64 = self.asks.iter().take(levels).map(|l| l.price * l.qty).sum();
        if ask_vol < f64::EPSILON {
            return 2.0;
        }
        bid_vol / ask_vol
    }

    pub fn spread_pct(&self) -> Option<f64> {
        let bid = self.bids.first()?.price;
        let ask = self.asks.first()?.price;
        if bid <= 0.0 {
            return None;
        }
        Some((ask - bid) / bid * 100.0)
    }

    pub fn best_bid(&self) -> Option<f64> {
        self.bids.first().map(|l| l.price)
    }

    pub fn best_ask(&self) -> Option<f64> {
        self.asks.first().map(|l| l.price)
    }
}

pub struct OrderBookTracker {
    books: HashMap<String, OrderBook>,
}

impl OrderBookTracker {
    pub fn new() -> Self {
        Self {
            books: HashMap::new(),
        }
    }

    pub fn apply_snapshot(&mut self, symbol: &str, bids: &[Level], asks: &[Level]) {
        let book = self.books.entry(symbol.to_string()).or_insert_with(|| OrderBook::new(10));
        book.apply_snapshot(bids, asks);
    }

    pub fn apply_update(&mut self, symbol: &str, bids: &[Level], asks: &[Level]) {
        if let Some(book) = self.books.get_mut(symbol) {
            book.apply_update(bids, asks);
        }
    }

    pub fn imbalance(&self, symbol: &str, levels: usize) -> Option<f64> {
        self.books.get(symbol).map(|b| b.imbalance(levels))
    }

    pub fn spread_pct(&self, symbol: &str) -> Option<f64> {
        self.books.get(symbol).and_then(|b| b.spread_pct())
    }
}
