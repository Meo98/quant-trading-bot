use crate::api::rest_client::KrakenRestClient;
use crate::trading::types::ExecutionEvent;
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration, timeout};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

const WS_AUTH_URL: &str = "wss://ws-auth.kraken.com/v2";
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(30);
const TOKEN_REFRESH: Duration = Duration::from_secs(600);

pub struct KrakenWsPrivate {
    api: KrakenRestClient,
    tx: mpsc::Sender<ExecutionEvent>,
}

impl KrakenWsPrivate {
    pub fn new(api: KrakenRestClient, tx: mpsc::Sender<ExecutionEvent>) -> Self {
        Self { api, tx }
    }

    async fn get_token(&self) -> Result<String> {
        let result = self
            .api
            .private_request("/0/private/GetWebSocketsToken", vec![])
            .await?;
        result
            .get("token")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("No token in response"))
    }

    pub async fn run(&self) {
        let mut backoff = 1u64;

        loop {
            log::info!("WS-Private: connecting...");
            match self.connect_and_run().await {
                Ok(_) => {
                    log::warn!("WS-Private: connection closed cleanly");
                    backoff = 1;
                }
                Err(e) => {
                    log::error!("WS-Private: error: {}", e);
                    backoff = (backoff * 2).min(30);
                }
            }
            log::info!("WS-Private: reconnecting in {}s...", backoff);
            sleep(Duration::from_secs(backoff)).await;
        }
    }

    async fn connect_and_run(&self) -> Result<()> {
        let token = self.get_token().await?;
        log::info!("WS-Private: got auth token");

        let (ws_stream, _) = connect_async(WS_AUTH_URL).await?;
        let (mut write, mut read) = ws_stream.split();

        let sub = json!({
            "method": "subscribe",
            "params": {
                "channel": "executions",
                "token": token,
                "snap_trades": false,
            }
        });
        write.send(Message::Text(sub.to_string())).await?;
        log::info!("WS-Private: subscribed to executions");

        let token_time = std::time::Instant::now();

        loop {
            if token_time.elapsed() > TOKEN_REFRESH {
                log::info!("WS-Private: token expired, reconnecting...");
                return Ok(());
            }

            match timeout(HEARTBEAT_TIMEOUT, read.next()).await {
                Ok(Some(Ok(Message::Text(text)))) => {
                    if let Err(e) = self.handle_message(&text).await {
                        log::debug!("WS-Private: parse error: {}", e);
                    }
                }
                Ok(Some(Ok(Message::Ping(data)))) => {
                    write.send(Message::Pong(data)).await?;
                }
                Ok(Some(Ok(_))) => {}
                Ok(Some(Err(e))) => return Err(e.into()),
                Ok(None) => return Ok(()),
                Err(_) => {
                    log::warn!("WS-Private: heartbeat timeout");
                    return Err(anyhow::anyhow!("heartbeat timeout"));
                }
            }
        }
    }

    async fn handle_message(&self, text: &str) -> Result<()> {
        let msg: Value = serde_json::from_str(text)?;

        let channel = msg.get("channel").and_then(|c| c.as_str()).unwrap_or("");
        let msg_type = msg.get("type").and_then(|t| t.as_str()).unwrap_or("");

        if channel != "executions" || msg_type == "heartbeat" {
            return Ok(());
        }

        let data = match msg.get("data").and_then(|d| d.as_array()) {
            Some(d) => d,
            None => return Ok(()),
        };

        for item in data {
            let exec_type = item
                .get("exec_type")
                .and_then(|e| e.as_str())
                .unwrap_or("");

            let ord_status = item
                .get("ord_status")
                .and_then(|s| s.as_str())
                .unwrap_or("");

            if exec_type != "trade" && exec_type != "canceled" && exec_type != "expired" {
                continue;
            }

            let event = ExecutionEvent {
                order_id: item
                    .get("order_id")
                    .and_then(|o| o.as_str())
                    .unwrap_or("")
                    .to_string(),
                exec_type: exec_type.to_string(),
                symbol: item
                    .get("symbol")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string(),
                side: item
                    .get("side")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string(),
                avg_price: item
                    .get("avg_price")
                    .and_then(|p| p.as_str())
                    .and_then(|s| s.parse().ok())
                    .or_else(|| item.get("avg_price").and_then(|p| p.as_f64()))
                    .unwrap_or(0.0),
                cum_qty: item
                    .get("cum_qty")
                    .and_then(|q| q.as_str())
                    .and_then(|s| s.parse().ok())
                    .or_else(|| item.get("cum_qty").and_then(|q| q.as_f64()))
                    .unwrap_or(0.0),
                fee: item
                    .get("fee")
                    .and_then(|f| f.as_str())
                    .and_then(|s| s.parse().ok())
                    .or_else(|| item.get("fee").and_then(|f| f.as_f64()))
                    .unwrap_or(0.0),
                ord_status: ord_status.to_string(),
                order_type: item
                    .get("order_type")
                    .and_then(|o| o.as_str())
                    .unwrap_or("")
                    .to_string(),
            };

            log::info!(
                "EXECUTION: {} {} {} | price={:.6} qty={:.6} fee={:.4} | status={}",
                event.exec_type,
                event.side,
                event.symbol,
                event.avg_price,
                event.cum_qty,
                event.fee,
                event.ord_status,
            );

            let _ = self.tx.send(event).await;
        }

        Ok(())
    }
}
