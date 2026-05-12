use serde::Deserialize;

#[derive(Debug, Clone)]
pub enum MarketEvent {
    Ticker {
        symbol: String,
        last: f64,
        bid: f64,
        ask: f64,
        volume: f64,
        high: f64,
        low: f64,
    },
    Trade {
        symbol: String,
        price: f64,
        qty: f64,
        side: TradeSide,
    },
    BookSnapshot {
        symbol: String,
        bids: Vec<Level>,
        asks: Vec<Level>,
    },
    BookUpdate {
        symbol: String,
        bids: Vec<Level>,
        asks: Vec<Level>,
    },
    Disconnected,
}

#[derive(Debug, Clone, Copy)]
pub enum TradeSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Level {
    pub price: f64,
    pub qty: f64,
}

#[derive(Debug, Clone)]
pub struct TradeSignal {
    pub pair: String,
    pub kraken_pair: String,
    pub price: f64,
    pub strength: f64,
    pub atr_pct: f64,
    pub components: Vec<SignalComponent>,
}

#[derive(Debug, Clone, Copy)]
pub enum SignalComponent {
    RsiBounce(f64),
    MomentumBreakout(f64),
    EmaAlignment,
    BookImbalance(f64),
    MultiTfAgreement(u8),
    VolumeSurge(f64),
    SpreadTight(f64),
    AdxStrong(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Timeframe {
    M1,
    M5,
    M15,
}

impl Timeframe {
    pub fn secs(&self) -> u64 {
        match self {
            Timeframe::M1 => 60,
            Timeframe::M5 => 300,
            Timeframe::M15 => 900,
        }
    }

    pub fn all() -> &'static [Timeframe] {
        &[Timeframe::M1, Timeframe::M5, Timeframe::M15]
    }
}

#[derive(Debug, Clone)]
pub struct Candle {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct ExecutionEvent {
    pub order_id: String,
    pub exec_type: String,
    pub symbol: String,
    pub side: String,
    pub avg_price: f64,
    pub cum_qty: f64,
    pub fee: f64,
    pub ord_status: String,
    pub order_type: String,
}

pub const STABLECOINS: &[&str] = &[
    "USDT/EUR", "USDC/EUR", "DAI/EUR", "PYUSD/EUR", "FDUSD/EUR",
    "TUSD/EUR", "BUSD/EUR", "GUSD/EUR", "USDP/EUR", "EURT/EUR",
    "EUROC/EUR", "STBL/EUR",
];
