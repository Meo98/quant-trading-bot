use crate::trading::types::{Level, MarketEvent, TradeSide};
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration, timeout};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

const WS_URL: &str = "wss://ws.kraken.com/v2";
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_SYMBOLS_PER_SUB: usize = 50;

pub struct KrakenWsClient {
    symbols: Vec<String>,
    tx: mpsc::Sender<MarketEvent>,
}

impl KrakenWsClient {
    pub fn new(symbols: Vec<String>, tx: mpsc::Sender<MarketEvent>) -> Self {
        Self { symbols, tx }
    }

    pub async fn run(&self) {
        let mut backoff = 1u64;

        loop {
            log::info!("WS: connecting to Kraken...");
            match self.connect_and_run().await {
                Ok(_) => {
                    log::warn!("WS: connection closed cleanly");
                    backoff = 1;
                }
                Err(e) => {
                    log::error!("WS: connection error: {}", e);
                    backoff = (backoff * 2).min(30);
                }
            }

            let _ = self.tx.send(MarketEvent::Disconnected).await;
            log::info!("WS: reconnecting in {}s...", backoff);
            sleep(Duration::from_secs(backoff)).await;
        }
    }

    async fn connect_and_run(&self) -> Result<()> {
        let (ws_stream, _) = connect_async(WS_URL).await?;
        let (mut write, mut read) = ws_stream.split();

        log::info!("WS: connected, subscribing to {} symbols...", self.symbols.len());

        for chunk in self.symbols.chunks(MAX_SYMBOLS_PER_SUB) {
            let symbols: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();

            let ticker_sub = json!({
                "method": "subscribe",
                "params": { "channel": "ticker", "symbol": symbols }
            });
            write.send(Message::Text(ticker_sub.to_string())).await?;

            let book_sub = json!({
                "method": "subscribe",
                "params": { "channel": "book", "symbol": symbols, "depth": 10 }
            });
            write.send(Message::Text(book_sub.to_string())).await?;

            let trade_sub = json!({
                "method": "subscribe",
                "params": { "channel": "trade", "symbol": symbols }
            });
            write.send(Message::Text(trade_sub.to_string())).await?;
        }

        log::info!("WS: subscribed to ticker, book, trade channels");

        let mut msg_count = 0u64;
        loop {
            match timeout(HEARTBEAT_TIMEOUT, read.next()).await {
                Ok(Some(Ok(Message::Text(text)))) => {
                    msg_count += 1;
                    if msg_count % 5000 == 0 {
                        log::info!("WS: {} messages processed", msg_count);
                    }
                    if let Err(e) = self.handle_message(&text).await {
                        log::debug!("WS: parse error: {}", e);
                    }
                }
                Ok(Some(Ok(Message::Ping(data)))) => {
                    write.send(Message::Pong(data)).await?;
                }
                Ok(Some(Ok(_))) => {}
                Ok(Some(Err(e))) => return Err(e.into()),
                Ok(None) => return Ok(()),
                Err(_) => {
                    log::warn!("WS: heartbeat timeout");
                    return Err(anyhow::anyhow!("heartbeat timeout"));
                }
            }
        }
    }

    async fn handle_message(&self, text: &str) -> Result<()> {
        let msg: Value = serde_json::from_str(text)?;

        let channel = msg.get("channel").and_then(|c| c.as_str()).unwrap_or("");
        let msg_type = msg.get("type").and_then(|t| t.as_str()).unwrap_or("");

        if msg_type == "heartbeat" || channel == "heartbeat" || channel == "status" {
            return Ok(());
        }

        let data = match msg.get("data").and_then(|d| d.as_array()) {
            Some(d) => d,
            None => return Ok(()),
        };

        for item in data {
            let event = match channel {
                "ticker" if msg_type == "update" || msg_type == "snapshot" => {
                    Self::parse_ticker(item)
                }
                "trade" if msg_type == "update" || msg_type == "snapshot" => {
                    Self::parse_trade(item)
                }
                "book" => Self::parse_book(item, msg_type),
                _ => None,
            };

            if let Some(event) = event {
                if let Err(e) = self.tx.try_send(event) {
                    match e {
                        mpsc::error::TrySendError::Full(_) => {
                            log::warn!("WS: market event channel full — dropping event");
                        }
                        mpsc::error::TrySendError::Closed(_) => {
                            log::error!("WS: market event channel closed");
                            return Err(anyhow::anyhow!("channel closed"));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn parse_ticker(data: &Value) -> Option<MarketEvent> {
        Some(MarketEvent::Ticker {
            symbol: data.get("symbol")?.as_str()?.to_string(),
            last: data.get("last")?.as_f64()?,
            bid: data.get("bid")?.as_f64()?,
            ask: data.get("ask")?.as_f64()?,
            volume: data.get("volume")?.as_f64()?,
            high: data.get("high")?.as_f64()?,
            low: data.get("low")?.as_f64()?,
        })
    }

    fn parse_trade(data: &Value) -> Option<MarketEvent> {
        let side_str = data.get("side")?.as_str()?;
        let side = if side_str == "buy" {
            TradeSide::Buy
        } else {
            TradeSide::Sell
        };
        Some(MarketEvent::Trade {
            symbol: data.get("symbol")?.as_str()?.to_string(),
            price: data.get("price")?.as_f64()?,
            qty: data.get("qty")?.as_f64()?,
            side,
        })
    }

    fn parse_book(data: &Value, msg_type: &str) -> Option<MarketEvent> {
        let symbol = data.get("symbol")?.as_str()?.to_string();
        let bids = Self::parse_levels(data.get("bids")?)?;
        let asks = Self::parse_levels(data.get("asks")?)?;

        if msg_type == "snapshot" {
            Some(MarketEvent::BookSnapshot {
                symbol,
                bids,
                asks,
            })
        } else {
            Some(MarketEvent::BookUpdate {
                symbol,
                bids,
                asks,
            })
        }
    }

    fn parse_levels(arr: &Value) -> Option<Vec<Level>> {
        let items = arr.as_array()?;
        let mut levels = Vec::with_capacity(items.len());
        for item in items {
            let price = item.get("price")?.as_f64()?;
            let qty = item.get("qty")?.as_f64()?;
            levels.push(Level { price, qty });
        }
        Some(levels)
    }
}
