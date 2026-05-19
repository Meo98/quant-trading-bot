use crate::api::rest_client::KrakenRestClient;
use crate::config::BotConfig;
use crate::trading::indicators::{Indicators, PriceBar};
use crate::trading::shared_state::SharedIndicators;
use crate::trading::types::{Candle, ExecutionEvent, TradeSignal};
use crate::trading::OpenTrade;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

const RSI_PERIOD: usize = 14;

const MIN_STAKE_EUR: f64 = 8.0;
const BASE_RISK_PCT: f64 = 0.15;
const MAX_RISK_PCT: f64 = 0.35;
const SIGNAL_SCALE_MIN: f64 = 3.0;
const SIGNAL_SCALE_MAX: f64 = 7.0;

pub struct TradingEngine {
    pub config: BotConfig,
    pub api: KrakenRestClient,
    pub open_trades: HashMap<String, OpenTrade>,
    pub eur_balance: f64,
    pub all_eur_pairs: HashMap<String, String>,
    pub pair_decimals: HashMap<String, u8>,
    pub asset_to_pair: HashMap<String, (String, String)>,
    pub indicators: HashMap<String, Indicators>,
    pub liquid_pairs: Vec<(String, String, f64)>,
    pub cooldowns: HashMap<String, u64>,
    /// Per-pair rate-limit for SL-recovery attempts in update_trailing_stops_ws.
    /// Without this, a permanently-broken trade (e.g. balance mismatch) causes
    /// the recovery to retry every 10s forever, spamming the log and API quota.
    pub sl_recovery_last_attempt: HashMap<String, u64>,
    /// Number of trades opened so far today. Reset by reset_daily_if_needed.
    /// Capped by config.max_trades_per_day to prevent fee-burn on noisy days.
    pub daily_trade_count: u32,
    pub daily_start_balance: f64,
    pub daily_pnl: f64,
    pub last_daily_reset: u64,
    pub tick_count: u64,
    pub shared: Option<SharedIndicators>,
}

impl TradingEngine {
    pub fn new(config: BotConfig) -> Self {
        let api = KrakenRestClient::new(config.api_key.clone(), config.api_secret.clone());
        Self {
            config,
            api,
            open_trades: HashMap::new(),
            eur_balance: 0.0,
            all_eur_pairs: HashMap::new(),
            pair_decimals: HashMap::new(),
            asset_to_pair: HashMap::new(),
            indicators: HashMap::new(),
            liquid_pairs: Vec::new(),
            cooldowns: HashMap::new(),
            sl_recovery_last_attempt: HashMap::new(),
            daily_trade_count: 0,
            daily_start_balance: 0.0,
            daily_pnl: 0.0,
            last_daily_reset: 0,
            tick_count: 0,
            shared: None,
        }
    }

    pub fn set_shared(&mut self, shared: SharedIndicators) {
        self.shared = Some(shared);
    }

    fn now_sec() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    pub async fn start(&mut self) -> Result<()> {
        log::info!("Starting engine...");
        self.fetch_eur_pairs().await?;
        self.fetch_balance().await?;
        self.sync_existing_positions().await;
        self.daily_start_balance = self.eur_balance + self.open_trades.values().map(|t| t.stake_eur).sum::<f64>();
        self.last_daily_reset = Self::now_sec();
        log::info!(
            "Engine ready: {} pairs, €{:.2} balance",
            self.all_eur_pairs.len(),
            self.eur_balance
        );
        Ok(())
    }


    fn pairs_match(a: &str, b: &str) -> bool {
        if a == b {
            return true;
        }
        let normalize = |p: &str| -> String {
            let p = p.to_uppercase();
            let base = p.strip_suffix("ZEUR")
                .or_else(|| p.strip_suffix("EUR"))
                .unwrap_or(&p);
            let base = base.strip_prefix('X')
                .filter(|s| s.len() >= 3)
                .unwrap_or(base);
            base.to_string()
        };
        normalize(a) == normalize(b)
    }

    async fn find_sl_order_for_pair(&self, kraken_pair: &str) -> Option<(String, f64)> {
        let orders = match self.api.private_request("/0/private/OpenOrders", vec![]).await {
            Ok(Value::Object(o)) => o,
            _ => return None,
        };
        let open = match orders.get("open") {
            Some(Value::Object(o)) => o,
            _ => return None,
        };
        for (txid, info) in open {
            let descr = match info.get("descr") {
                Some(d) => d,
                None => continue,
            };
            let pair = descr.get("pair").and_then(|p| p.as_str()).unwrap_or("");
            let otype = descr.get("ordertype").and_then(|o| o.as_str()).unwrap_or("");
            let direction = descr.get("type").and_then(|t| t.as_str()).unwrap_or("");

            if Self::pairs_match(pair, kraken_pair) && otype == "stop-loss" && direction == "sell" {
                let price = descr.get("price")
                    .and_then(|p| p.as_str())
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                return Some((txid.clone(), price));
            }
        }
        None
    }

    /// Check if an asset (e.g. "LTC", "XDC") is held in a Kraken Earn allocation.
    /// Returns Some((strategy_id, amount_allocated)) if yes — useful for emitting
    /// actionable error messages when "Insufficient funds" is caused by Earn lock
    /// rather than open orders. The bot CANNOT deallocate via API for
    /// `opt_in_rewards` strategies; user must use the Kraken web UI.
    async fn check_earn_allocation(&self, asset_symbol: &str) -> Option<(String, f64)> {
        let result = self
            .api
            .private_request("/0/private/Earn/Allocations", vec![])
            .await
            .ok()?;
        let items = result.get("items")?.as_array()?;
        for item in items {
            let asset = item.get("native_asset")?.as_str()?;
            if asset.eq_ignore_ascii_case(asset_symbol) {
                let strategy = item.get("strategy_id")?.as_str()?.to_string();
                let amount: f64 = item
                    .get("amount_allocated")?
                    .get("total")?
                    .get("native")?
                    .as_str()?
                    .parse()
                    .ok()?;
                if amount > 0.0001 {
                    return Some((strategy, amount));
                }
            }
        }
        None
    }

    async fn cancel_open_orders_for_pair(&self, kraken_pair: &str) {
        let orders = match self.api.private_request("/0/private/OpenOrders", vec![]).await {
            Ok(Value::Object(o)) => o,
            _ => return,
        };
        let open = match orders.get("open") {
            Some(Value::Object(o)) => o,
            _ => return,
        };
        for (txid, info) in open {
            let pair = info.get("descr")
                .and_then(|d| d.get("pair"))
                .and_then(|p| p.as_str())
                .unwrap_or("");
            if Self::pairs_match(pair, kraken_pair) {
                log::info!("Cancelling old order {} for {}", txid, kraken_pair);
                let _ = self.cancel_order(txid).await;
            }
        }
    }

    async fn sync_existing_positions(&mut self) {
        let balances = match self.api.private_request("/0/private/Balance", vec![]).await {
            Ok(Value::Object(b)) => b,
            _ => return,
        };

        for (asset, val) in &balances {
            if asset == "ZEUR" || asset == "EUR" || asset == "ZUSD" || asset == "USD" {
                continue;
            }
            let amount: f64 = val.as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
            if amount <= 0.0 {
                continue;
            }

            let (display, kraken) = match self.asset_to_pair.get(asset) {
                Some(p) => p.clone(),
                None => continue,
            };

            if self.open_trades.contains_key(&display) {
                continue;
            }

            let ticker = self.api.public_request(
                "/0/public/Ticker",
                &[("pair", &kraken)],
            ).await.ok();
            let price = ticker.as_ref()
                .and_then(|t| t.as_object())
                .and_then(|m| m.values().next())
                .and_then(|v| v.get("c"))
                .and_then(|c| c.as_array())
                .and_then(|a| a.first())
                .and_then(|p| p.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            let stake = amount * price;
            if stake < 1.0 {
                continue;
            }

            if crate::trading::types::STABLECOINS.contains(&display.as_str()) {
                log::info!("Selling stablecoin position: {} | {:.4} units ≈ €{:.2}", display, amount, stake);
                self.cancel_open_orders_for_pair(&kraken).await;
                let _ = self.api.private_request(
                    "/0/private/AddOrder",
                    vec![
                        ("pair", kraken.clone()),
                        ("type", "sell".to_string()),
                        ("ordertype", "market".to_string()),
                        ("volume", format!("{:.8}", amount)),
                    ],
                ).await;
                continue;
            }

            log::info!("Synced position: {} | {:.4} units @ {:.6} ≈ €{:.2}", display, amount, price, stake);

            let entry_atr = 0.02;
            let hard_sl_price = price * (1.0 - (entry_atr * self.config.hard_sl_atr_mult).clamp(0.03, 0.20));

            let mut trade = OpenTrade {
                pair: display.clone(),
                kraken_pair: kraken.clone(),
                entry_price: price,
                amount,
                stake_eur: stake,
                highest_price: price,
                entry_time: Self::now_sec(),
                stop_loss_order_txid: None,
                server_stop_price: 0.0,
                entry_atr,
                exit_reason: None,
            };

            if let Some((txid, sl_price)) = self.find_sl_order_for_pair(&kraken).await {
                log::info!("Adopted existing SL for {}: {:.6} ({})", display, sl_price, txid);
                trade.stop_loss_order_txid = Some(txid);
                trade.server_stop_price = sl_price;
            } else {
                match self.place_stop_loss(&kraken, amount, hard_sl_price).await {
                    Ok(txid) => {
                        log::info!("Placed SL for synced {}: {:.6} ({})", display, hard_sl_price, txid);
                        trade.stop_loss_order_txid = Some(txid);
                        trade.server_stop_price = hard_sl_price;
                    }
                    Err(e) => log::warn!("SL placement failed for synced {}: {}", display, e),
                }
            }

            self.open_trades.insert(display, trade);
        }

        if !self.open_trades.is_empty() {
            log::info!("Synced {} existing positions", self.open_trades.len());
        }
    }

    async fn fetch_eur_pairs(&mut self) -> Result<()> {
        let result = self.api.public_request("/0/public/AssetPairs", &[]).await?;
        if let Value::Object(pairs) = result {
            self.all_eur_pairs.clear();
            self.pair_decimals.clear();
            self.asset_to_pair.clear();
            for (kraken_pair, info) in pairs {
                if let Some(quote) = info.get("quote").and_then(|q| q.as_str()) {
                    if (quote == "ZEUR" || quote == "EUR") && !kraken_pair.contains(".d") {
                        let wsname = info
                            .get("wsname")
                            .and_then(|w| w.as_str())
                            .unwrap_or(&kraken_pair);
                        let decimals = info
                            .get("pair_decimals")
                            .and_then(|d| d.as_u64())
                            .unwrap_or(8) as u8;
                        if let Some(base) = info.get("base").and_then(|b| b.as_str()) {
                            self.asset_to_pair.insert(
                                base.to_string(),
                                (wsname.to_string(), kraken_pair.clone()),
                            );
                        }
                        self.all_eur_pairs
                            .insert(wsname.to_string(), kraken_pair.clone());
                        self.pair_decimals.insert(kraken_pair.clone(), decimals);
                    }
                }
            }
            log::info!("Loaded {} EUR pairs", self.all_eur_pairs.len());
        }
        Ok(())
    }

    pub async fn fetch_ohlc(
        &self,
        kraken_pair: &str,
        interval_min: u64,
    ) -> Result<Vec<Candle>> {
        let interval_str = interval_min.to_string();
        let result = self
            .api
            .public_request(
                "/0/public/OHLC",
                &[("pair", kraken_pair), ("interval", &interval_str)],
            )
            .await?;

        let mut candles = Vec::new();
        if let Value::Object(data) = result {
            for (key, arr) in &data {
                if key == "last" {
                    continue;
                }
                if let Value::Array(rows) = arr {
                    for row in rows {
                        if let Value::Array(fields) = row {
                            if fields.len() < 7 {
                                continue;
                            }
                            let ts = fields[0].as_u64().unwrap_or(0);
                            let open = fields[1].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                            let high = fields[2].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                            let low = fields[3].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                            let close = fields[4].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                            let volume = fields[6].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                            if close > 0.0 {
                                candles.push(Candle {
                                    timestamp: ts,
                                    open,
                                    high,
                                    low,
                                    close,
                                    volume,
                                });
                            }
                        }
                    }
                }
            }
        }
        candles.sort_by_key(|c| c.timestamp);
        Ok(candles)
    }

    pub fn top_liquid_pairs(&self, count: usize) -> Vec<(String, String)> {
        let mut pairs: Vec<(String, String)> = self
            .all_eur_pairs
            .iter()
            .map(|(ws, kr)| (ws.clone(), kr.clone()))
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        pairs.truncate(count);
        pairs
    }

    pub async fn fetch_balance(&mut self) -> Result<()> {
        let result = self.api.private_request("/0/private/Balance", vec![]).await?;
        if let Value::Object(balances) = result {
            self.eur_balance = balances
                .get("ZEUR")
                .or_else(|| balances.get("EUR"))
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            if !self.open_trades.is_empty() {
                let mut pairs_with_balance: HashMap<String, f64> = HashMap::new();
                for (asset, val) in &balances {
                    if asset == "ZEUR" || asset == "EUR" || asset == "ZUSD" || asset == "USD" {
                        continue;
                    }
                    let amount: f64 = val.as_str()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0.0);
                    if amount <= 0.0 {
                        continue;
                    }
                    if let Some((display, _)) = self.asset_to_pair.get(asset) {
                        pairs_with_balance.insert(display.clone(), amount);
                    }
                }

                let mut ghosts = Vec::new();
                for (pair, trade) in &self.open_trades {
                    match pairs_with_balance.get(pair) {
                        Some(&bal) if bal >= trade.amount * 0.5 => {}
                        _ => ghosts.push(pair.clone()),
                    }
                }

                for pair in ghosts {
                    if let Some(trade) = self.open_trades.remove(&pair) {
                        log::warn!(
                            "GHOST detected: {} — asset gone from Kraken (server SL triggered). \
                             Removing after {}m. Entry: {:.6}, Stake: €{:.2}",
                            pair, trade.age_minutes(), trade.entry_price, trade.stake_eur
                        );
                        self.cooldowns.insert(pair, Self::now_sec() + 3600);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn reset_daily_if_needed(&mut self) {
        let now = Self::now_sec();
        let total_balance = self.eur_balance
            + self.open_trades.values().map(|t| t.stake_eur).sum::<f64>();

        // 1. Regular daily reset (24h elapsed)
        if now - self.last_daily_reset >= 86400 {
            log::info!(
                "Daily reset | Previous: €{:.2} → Now: €{:.2} | Day P&L: {:.2} | Trades today: {}",
                self.daily_start_balance,
                total_balance,
                self.daily_pnl,
                self.daily_trade_count
            );
            self.daily_start_balance = total_balance.max(self.eur_balance);
            self.daily_pnl = 0.0;
            self.daily_trade_count = 0;
            self.last_daily_reset = now;
            return;
        }

        // 2. Deposit/withdrawal detection: total_balance moved more than
        //    daily_pnl can account for. Rebases so daily-drawdown limit
        //    stays meaningful after capital changes. Threshold is the
        //    larger of €20 absolute or 10% of current total — small enough
        //    to catch real deposits, large enough to ignore ghost-position
        //    adjustments and price drift between fetch_balance ticks.
        let expected = self.daily_start_balance + self.daily_pnl;
        let unexplained = total_balance - expected;
        let threshold = (total_balance * 0.10).max(20.0);

        if unexplained.abs() > threshold {
            log::info!(
                "Deposit/withdrawal detected: total €{:.2} vs expected €{:.2} \
                 (delta {:+.2}, threshold €{:.2}) — rebasing daily_start_balance",
                total_balance, expected, unexplained, threshold
            );
            self.daily_start_balance = total_balance.max(self.eur_balance);
            self.daily_pnl = 0.0;
        }
    }

    fn is_daily_drawdown_ok(&self) -> bool {
        if self.daily_start_balance < 5.0 {
            return true;
        }
        let drawdown_pct = self.daily_pnl / self.daily_start_balance;
        drawdown_pct > -self.config.max_daily_drawdown
    }

    /// Collect ticker data for all pairs and update indicators.
    /// Also selects the top liquid pairs for analysis.
    pub async fn collect_tickers(&mut self) -> Result<()> {
        let pair_list: String = self
            .all_eur_pairs
            .values()
            .cloned()
            .collect::<Vec<_>>()
            .join(",");

        let result = self
            .api
            .public_request("/0/public/Ticker", &[("pair", &pair_list)])
            .await?;

        let tickers = match result {
            Value::Object(t) => t,
            _ => return Err(anyhow!("Invalid ticker response")),
        };

        let now = Self::now_sec();
        let mut volumes: Vec<(String, String, f64, f64)> = Vec::new();

        for (display_name, kraken_pair) in &self.all_eur_pairs {
            let ticker = match tickers.get(kraken_pair) {
                Some(t) => t,
                None => continue,
            };

            let price = Self::parse_ticker_field(ticker, "c", 0).unwrap_or(0.0);
            let volume_24h = Self::parse_ticker_field(ticker, "v", 1).unwrap_or(0.0);

            if price <= 0.0 {
                continue;
            }

            let volume_eur = volume_24h * price;

            let ind = self
                .indicators
                .entry(display_name.clone())
                .or_insert_with(|| Indicators::with_capacity(500));
            ind.push(PriceBar {
                timestamp: now,
                close: price,
                high: price,
                low: price,
                volume: volume_eur,
            });

            if volume_eur >= self.config.min_volume_eur {
                volumes.push((
                    display_name.clone(),
                    kraken_pair.clone(),
                    volume_eur,
                    price,
                ));
            }
        }

        volumes.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        self.liquid_pairs = volumes
            .into_iter()
            .take(self.config.max_watched_pairs)
            .map(|(d, k, v, _)| (d, k, v))
            .collect();

        Ok(())
    }

    fn parse_ticker_field(ticker: &Value, field: &str, index: usize) -> Option<f64> {
        ticker
            .get(field)
            .and_then(|f| f.get(index))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
    }




    /// Sends a market-sell for the given trade. Extracted into a helper so the
    /// retry path in execute_sell doesn't have to duplicate the request.
    async fn send_market_sell(&self, trade: &OpenTrade) -> Result<Value> {
        self.api
            .private_request(
                "/0/private/AddOrder",
                vec![
                    ("pair", trade.kraken_pair.clone()),
                    ("type", "sell".to_string()),
                    ("ordertype", "market".to_string()),
                    ("volume", format!("{:.8}", trade.amount)),
                ],
            )
            .await
    }

    pub async fn execute_sell(&mut self, pair: &str, reason: &str) -> Result<bool> {
        let trade = match self.open_trades.get(pair) {
            Some(t) => t.clone(),
            None => return Ok(false),
        };

        // Step 1: cancel any SL order that may be holding the asset.
        if let Some(ref txid) = trade.stop_loss_order_txid {
            match self.cancel_order(txid).await {
                Ok(_) => log::info!("Cancelled SL {} for {}", txid, pair),
                Err(_) => {
                    if let Some((found_txid, _)) = self.find_sl_order_for_pair(&trade.kraken_pair).await {
                        let _ = self.cancel_order(&found_txid).await;
                        log::info!("Cancelled found SL {} for {}", found_txid, pair);
                    }
                }
            }
        } else {
            self.cancel_open_orders_for_pair(&trade.kraken_pair).await;
        }

        // Step 2: wait for Kraken to release the asset volume after the cancel.
        // Without this sleep, the immediately-following AddOrder request races
        // ahead of Kraken's internal balance update and hits "Insufficient funds"
        // even though the cancel was accepted. Empirically 800 ms is enough.
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;

        // Step 3: market sell.
        let mut result = self.send_market_sell(&trade).await;

        // Step 4: one full recovery + retry on "Insufficient funds". This covers
        // the case where there are additional stale orders for the pair (not just
        // the SL we cancelled), or where 800 ms wasn't enough for some reason.
        if let Err(ref e) = result {
            if e.to_string().contains("Insufficient funds") {
                log::warn!(
                    "Sell {} got Insufficient funds — broader cancel + 1.5s wait + retry",
                    pair
                );
                self.cancel_open_orders_for_pair(&trade.kraken_pair).await;
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                result = self.send_market_sell(&trade).await;
            }
        }

        match result {
            Ok(resp) => {
                let sell_price = self.get_ws_price(pair).await
                    .or_else(|| self.indicators.get(pair).and_then(|i| i.last_price()))
                    .unwrap_or(trade.entry_price);
                let pnl = trade.profit_eur(sell_price);
                self.daily_pnl += pnl;

                log::info!(
                    "SELL {} | {} | P&L: €{:.2} | Day: €{:.2} | {:?}",
                    pair,
                    reason,
                    pnl,
                    self.daily_pnl,
                    resp
                );

                self.open_trades.remove(pair);
                self.cooldowns
                    .insert(pair.to_string(), Self::now_sec() + 3600);
                Ok(true)
            }
            Err(e) => {
                let msg = e.to_string();
                log::error!("Sell failed {} (after retry): {}", pair, msg);
                if msg.contains("Insufficient funds") {
                    // Most common true cause: asset is held in Kraken Earn
                    // (opt_in_rewards auto-allocation). Check + give precise
                    // error so user knows where to look. Then drop the trade
                    // to break the exit-scan loop (would otherwise retry every
                    // 10s forever).
                    let asset_symbol = pair.split('/').next().unwrap_or(pair);
                    if let Some((strategy_id, amount)) = self.check_earn_allocation(asset_symbol).await {
                        log::error!(
                            "CRITICAL: {} is held in Kraken EARN (strategy={}, amount={:.6}). \
                             Bot cannot deallocate via API (opt_in_rewards strategies block API deallocate). \
                             FIX: kraken.com → Earn → manually deallocate {}. \
                             To prevent recurrence: Account Settings → opt out of Opt-In Rewards. \
                             Dropping trade to free slot.",
                            pair, strategy_id, amount, asset_symbol
                        );
                    } else {
                        log::error!(
                            "CRITICAL: {} permanently stuck (not in Earn, no open orders found). \
                             Check Kraken manually for margin positions or unknown order types. \
                             Dropping trade to break loop.",
                            pair
                        );
                    }
                    self.open_trades.remove(pair);
                    self.cooldowns
                        .insert(pair.to_string(), Self::now_sec() + 3600);
                }
                Err(e)
            }
        }
    }


    fn format_price(&self, kraken_pair: &str, price: f64) -> String {
        let decimals = self.pair_decimals.get(kraken_pair).copied().unwrap_or(8) as usize;
        format!("{:.prec$}", price, prec = decimals)
    }

    async fn place_stop_loss(
        &self,
        kraken_pair: &str,
        volume: f64,
        stop_price: f64,
    ) -> Result<String> {
        let result = self
            .api
            .private_request(
                "/0/private/AddOrder",
                vec![
                    ("pair", kraken_pair.to_string()),
                    ("type", "sell".to_string()),
                    ("ordertype", "stop-loss".to_string()),
                    ("price", self.format_price(kraken_pair, stop_price)),
                    ("volume", format!("{:.8}", volume)),
                ],
            )
            .await?;

        result
            .get("txid")
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("No txid in response: {:?}", result))
    }

    async fn edit_stop_loss(
        &self,
        old_txid: &str,
        kraken_pair: &str,
        new_stop_price: f64,
    ) -> Result<String> {
        let result = self
            .api
            .private_request(
                "/0/private/EditOrder",
                vec![
                    ("txid", old_txid.to_string()),
                    ("pair", kraken_pair.to_string()),
                    ("price", self.format_price(kraken_pair, new_stop_price)),
                ],
            )
            .await?;

        result
            .get("txid")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("No txid in EditOrder response: {:?}", result))
    }

    async fn cancel_order(&self, txid: &str) -> Result<()> {
        self.api
            .private_request("/0/private/CancelOrder", vec![("txid", txid.to_string())])
            .await?;
        Ok(())
    }

    /// Returns (vol_executed, cost) — the actually filled volume and EUR cost.
    /// Used to reconcile partial fills against requested order size.
    async fn query_order_fill(&self, txid: &str) -> Result<(f64, f64)> {
        let result = self
            .api
            .private_request("/0/private/QueryOrders", vec![("txid", txid.to_string())])
            .await?;
        let order = result
            .get(txid)
            .ok_or_else(|| anyhow!("order {} not in QueryOrders response", txid))?;
        let vol_exec = order
            .get("vol_exec")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| anyhow!("no vol_exec field"))?;
        let cost = order
            .get("cost")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .ok_or_else(|| anyhow!("no cost field"))?;
        Ok((vol_exec, cost))
    }

    pub async fn deadman_heartbeat(&self) {
        match self
            .api
            .private_request(
                "/0/private/CancelAllOrdersAfter",
                vec![("timeout", "90".to_string())],
            )
            .await
        {
            Ok(_) => log::debug!("Deadman heartbeat sent (90s)"),
            Err(e) => log::warn!("Deadman heartbeat failed: {}", e),
        }
    }

    pub fn handle_execution(&mut self, event: &ExecutionEvent) {
        if event.side == "sell" && event.order_type == "stop-loss"
            && (event.ord_status == "filled" || event.exec_type == "trade")
        {
            let mut removed_pair = None;
            for (pair, trade) in &self.open_trades {
                if let Some(ref txid) = trade.stop_loss_order_txid {
                    if txid == &event.order_id {
                        let pnl = trade.amount * event.avg_price - trade.stake_eur;
                        log::info!(
                            "SL TRIGGERED {} via WS | fill={:.6} qty={:.6} fee={:.4} | P&L: €{:.2}",
                            pair, event.avg_price, event.cum_qty, event.fee, pnl
                        );
                        self.daily_pnl += pnl;
                        removed_pair = Some(pair.clone());
                        break;
                    }
                }
            }
            if let Some(pair) = removed_pair {
                self.open_trades.remove(&pair);
                self.cooldowns.insert(pair, Self::now_sec() + 3600);
            }
        }
    }

    pub async fn update_peaks(&mut self) {
        for trade in self.open_trades.values_mut() {
            if let Some(ref shared) = self.shared {
                let state = shared.read().await;
                if let Some(snap) = state.get(&trade.pair) {
                    if snap.last_price > trade.highest_price {
                        trade.highest_price = snap.last_price;
                    }
                }
            }
        }
    }

    pub async fn get_ws_price(&self, pair: &str) -> Option<f64> {
        if let Some(ref shared) = self.shared {
            let state = shared.read().await;
            if let Some(snap) = state.get(pair) {
                if snap.last_price > 0.0 {
                    return Some(snap.last_price);
                }
            }
        }
        self.indicators.get(pair).and_then(|i| i.last_price())
    }

    pub async fn get_ws_rsi(&self, pair: &str) -> Option<f64> {
        if let Some(ref shared) = self.shared {
            let state = shared.read().await;
            if let Some(snap) = state.get(pair) {
                if let Some(rsi) = snap.rsi_5m {
                    return Some(rsi);
                }
            }
        }
        self.indicators.get(pair).and_then(|i| i.rsi(RSI_PERIOD))
    }

    fn compute_position_size(&self, signal: &TradeSignal) -> f64 {
        let total_capital = self.eur_balance
            + self.open_trades.values().map(|t| t.stake_eur).sum::<f64>();
        if total_capital < MIN_STAKE_EUR {
            return 0.0;
        }

        let strength_frac = ((signal.strength - SIGNAL_SCALE_MIN) / (SIGNAL_SCALE_MAX - SIGNAL_SCALE_MIN))
            .clamp(0.0, 1.0);
        let risk_pct = BASE_RISK_PCT + strength_frac * (MAX_RISK_PCT - BASE_RISK_PCT);

        let vol_adj = if signal.atr_pct > 0.05 {
            0.5
        } else if signal.atr_pct > 0.03 {
            0.75
        } else {
            1.0
        };

        let remaining = self.config.max_open_trades - self.open_trades.len();
        let max_per_slot = self.eur_balance / remaining.max(1) as f64;

        let stake = (total_capital * risk_pct * vol_adj).min(max_per_slot).min(self.eur_balance * 0.95);

        stake.max(0.0)
    }

    pub async fn execute_buy_from_signal(&mut self, signal: &TradeSignal) -> Result<bool> {
        if self.open_trades.contains_key(&signal.pair) {
            return Ok(false);
        }
        if self.open_trades.len() >= self.config.max_open_trades {
            return Ok(false);
        }
        if !self.is_daily_drawdown_ok() {
            return Ok(false);
        }
        // Daily trade cap — prevents overtrading on high-noise days where
        // many false signals would churn through fees. 0 = unlimited.
        if self.config.max_trades_per_day > 0
            && self.daily_trade_count >= self.config.max_trades_per_day
        {
            log::debug!(
                "Skip signal {}: daily trade cap reached ({}/{})",
                signal.pair, self.daily_trade_count, self.config.max_trades_per_day
            );
            return Ok(false);
        }

        let now = Self::now_sec();
        if let Some(&cd) = self.cooldowns.get(&signal.pair) {
            if now < cd {
                return Ok(false);
            }
        }

        let stake = self.compute_position_size(signal);
        if stake < MIN_STAKE_EUR {
            log::debug!("Skip signal {}: stake €{:.2} too small", signal.pair, stake);
            return Ok(false);
        }

        let amount = stake / signal.price;

        let result = self
            .api
            .private_request(
                "/0/private/AddOrder",
                vec![
                    ("pair", signal.kraken_pair.clone()),
                    ("type", "buy".to_string()),
                    ("ordertype", "market".to_string()),
                    ("volume", format!("{:.8}", amount)),
                ],
            )
            .await;

        match result {
            Ok(resp) => {
                log::info!(
                    "BUY {} | €{:.2} @ {:.6} | strength={:.1} ATR={:.2}% | components: {:?} | {:?}",
                    signal.pair, stake, signal.price, signal.strength,
                    signal.atr_pct * 100.0, signal.components, resp
                );

                // Reconcile against actual fill (partial-fill safe). Falls back
                // to requested values if QueryOrders fails — we still want the
                // trade tracked even if reconciliation isn't possible.
                let buy_txid = resp
                    .get("txid")
                    .and_then(|t| t.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let (actual_amount, actual_stake, actual_price) = if let Some(ref txid) = buy_txid {
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                    match self.query_order_fill(txid).await {
                        Ok((vol, cost)) if vol > 0.0 => {
                            let avg_price = cost / vol;
                            if (vol - amount).abs() > amount * 0.02 {
                                log::warn!(
                                    "PARTIAL FILL {}: requested {:.6} got {:.6} ({:.1}%) | cost €{:.2} vs €{:.2}",
                                    signal.pair, amount, vol, vol / amount * 100.0, cost, stake
                                );
                            }
                            (vol, cost, avg_price)
                        }
                        Ok(_) => {
                            log::warn!("QueryOrders {}: zero vol_exec, using requested", signal.pair);
                            (amount, stake, signal.price)
                        }
                        Err(e) => {
                            log::warn!("QueryOrders {} failed: {} — using requested values", signal.pair, e);
                            (amount, stake, signal.price)
                        }
                    }
                } else {
                    log::warn!("BUY {}: no txid in response — using requested values", signal.pair);
                    (amount, stake, signal.price)
                };

                let hard_sl_price = actual_price
                    * (1.0 - (signal.atr_pct * self.config.hard_sl_atr_mult).clamp(0.03, 0.20));

                let mut trade = OpenTrade {
                    pair: signal.pair.clone(),
                    kraken_pair: signal.kraken_pair.clone(),
                    entry_price: actual_price,
                    amount: actual_amount,
                    stake_eur: actual_stake,
                    highest_price: actual_price,
                    entry_time: now,
                    stop_loss_order_txid: None,
                    server_stop_price: 0.0,
                    entry_atr: signal.atr_pct,
                    exit_reason: None,
                };

                let mut sl_placed = false;
                for attempt in 0..2 {
                    match self
                        .place_stop_loss(&signal.kraken_pair, actual_amount, hard_sl_price)
                        .await
                    {
                        Ok(txid) => {
                            log::info!("Server SL for {}: {:.6} ({})", signal.pair, hard_sl_price, txid);
                            trade.stop_loss_order_txid = Some(txid);
                            trade.server_stop_price = hard_sl_price;
                            sl_placed = true;
                            break;
                        }
                        Err(e) => {
                            log::error!("SL placement failed for {} (attempt {}): {}", signal.pair, attempt + 1, e);
                            if attempt == 0 {
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            }
                        }
                    }
                }
                if !sl_placed {
                    log::error!("CRITICAL: {} has NO stop-loss protection! Recovery will retry in trail loop.", signal.pair);
                }

                self.open_trades.insert(signal.pair.clone(), trade);
                self.eur_balance -= actual_stake;
                self.cooldowns.insert(signal.pair.clone(), now + 3600);
                self.daily_trade_count += 1;
                Ok(true)
            }
            Err(e) => {
                log::error!("Buy failed {}: {}", signal.pair, e);
                if e.to_string().contains("Insufficient funds") {
                    self.eur_balance = 0.0;
                }
                Err(e)
            }
        }
    }

    pub async fn snapshot_prices_rsis(&self) -> (HashMap<String, f64>, HashMap<String, f64>) {
        let mut prices = HashMap::new();
        let mut rsis = HashMap::new();
        for pair in self.open_trades.keys() {
            if let Some(p) = self.get_ws_price(pair).await {
                prices.insert(pair.clone(), p);
            }
            if let Some(r) = self.get_ws_rsi(pair).await {
                rsis.insert(pair.clone(), r);
            }
        }
        (prices, rsis)
    }

    pub async fn scan_exits_ws(&mut self, prices: &HashMap<String, f64>, rsis: &HashMap<String, f64>) -> Vec<(String, String)> {
        let mut exits = Vec::new();

        let pairs: Vec<String> = self.open_trades.keys().cloned().collect();

        for pair in pairs {
            let trade = match self.open_trades.get_mut(&pair) {
                Some(t) => t,
                None => continue,
            };

            let price = match prices.get(&pair) {
                Some(&p) => p,
                None => continue,
            };

            if price > trade.highest_price {
                trade.highest_price = price;
            }

            let profit = trade.profit_pct(price);
            let profit_eur = trade.profit_eur(price);
            let age = trade.age_minutes();
            let atr = trade.entry_atr;
            let profit_in_atr = if atr > 0.0 { profit / atr } else { 0.0 };

            // 1. Hard stop-loss
            let hard_sl_pct = -(atr * self.config.hard_sl_atr_mult).clamp(0.03, 0.15);
            if profit <= hard_sl_pct {
                exits.push((
                    pair.clone(),
                    format!("HARD-SL: {:.1}% (limit {:.1}%) | €{:.2}", profit * 100.0, hard_sl_pct * 100.0, profit_eur),
                ));
                continue;
            }

            // 2. Breakeven stop: if price reached 1x ATR above entry but fell back
            let peak_profit_pct = (trade.highest_price - trade.entry_price) / trade.entry_price;
            let peak_in_atr = if atr > 0.0 { peak_profit_pct / atr } else { 0.0 };
            if peak_in_atr >= 1.0 && price <= trade.entry_price * 1.002 {
                exits.push((
                    pair.clone(),
                    format!("BREAKEVEN: peaked {:.1}x ATR, back at entry | €{:.2}", peak_in_atr, profit_eur),
                ));
                continue;
            }

            // 3. Progressive Chandelier trailing stop (tighter at higher profits)
            let trail_mult = if profit_in_atr >= 3.0 {
                0.5
            } else if profit_in_atr >= 2.0 {
                0.75
            } else if profit_in_atr >= 1.0 {
                self.config.trail_atr_mult
            } else {
                0.0
            };

            // RSI extreme: tighten trail further
            let trail_mult = if let Some(&rsi) = rsis.get(&pair) {
                if rsi > self.config.rsi_overbought && trail_mult > 0.0 {
                    trail_mult.min(0.5)
                } else {
                    trail_mult
                }
            } else {
                trail_mult
            };

            if trail_mult > 0.0 {
                let trail_pct = (atr * trail_mult).clamp(0.01, 0.10);
                let drawdown = trade.drawdown_from_high(price);
                if drawdown <= -trail_pct {
                    exits.push((
                        pair.clone(),
                        format!("TRAIL-STOP: {:.1}% from peak (trail {:.1}x ATR) | P/L: {:.1}% €{:.2}",
                            drawdown * 100.0, trail_mult, profit * 100.0, profit_eur),
                    ));
                    continue;
                }
            }

            // 4. Time stop — tier-based to avoid killing slightly-losing trades
            //    that might recover. Replaces old behavior of killing ANY trade
            //    < 1.0 ATR profit after max_hold (which was 2h, now 6h).
            //
            //    Tier 1 (max_hold_minutes, default 6h):
            //      Only kill clear losers (worse than -0.5 ATR).
            //      Stagnant near-entry trades get more time.
            //
            //    Tier 2 (2 × max_hold_minutes, 12h):
            //      Kill anything that's not a clear winner. At this point even
            //      slightly-positive trades are tying up capital for too little.
            if age > self.config.max_hold_minutes {
                let should_exit = if age > self.config.max_hold_minutes * 2 {
                    // Tier 2: kill stagnant trades to free the slot
                    profit_in_atr < 1.0
                } else {
                    // Tier 1: only kill clear losers
                    profit_in_atr < -0.5
                };
                if should_exit {
                    exits.push((
                        pair.clone(),
                        format!("TIME-STOP: {}h {}m | P/L: {:.1}% €{:.2}", age / 60, age % 60, profit * 100.0, profit_eur),
                    ));
                }
            }
        }

        exits
    }

    pub async fn update_trailing_stops_ws(&mut self, prices: &HashMap<String, f64>, rsis: &HashMap<String, f64>) {
        let pairs: Vec<String> = self.open_trades.keys().cloned().collect();

        for pair in pairs {
            // SL recovery for trades without a tracked stop-loss. Three-step
            // atomic strategy: (1) adopt any orphaned SL already on the book
            // for this pair, (2) if none, cancel stale pair-orders that may be
            // reserving volume and blocking placement, (3) place a fresh SL.
            let initial_sl_data = match self.open_trades.get(&pair) {
                Some(t) if t.stop_loss_order_txid.is_none() => {
                    let hs = t.entry_price
                        * (1.0
                            - (t.entry_atr * self.config.hard_sl_atr_mult).clamp(0.03, 0.20));
                    Some((t.kraken_pair.clone(), t.amount, hs))
                }
                Some(_) => None,
                None => continue,
            };

            if let Some((kraken_pair, amount, hard_stop)) = initial_sl_data {
                // Rate-limit: don't retry SL recovery for the same pair more often
                // than every 60s. Without this, a permanently-broken trade (e.g.
                // Kraken balance mismatch from partial fill, manual intervention,
                // or asset locked by unknown order) would loop the recovery every
                // 10s, spamming logs and burning API quota.
                let now = Self::now_sec();
                let throttle_seconds = 60u64;
                if let Some(&last) = self.sl_recovery_last_attempt.get(&pair) {
                    if now.saturating_sub(last) < throttle_seconds {
                        continue;
                    }
                }
                self.sl_recovery_last_attempt.insert(pair.clone(), now);

                // Step 1: adopt orphaned SL (free, safe — no API mutation)
                if let Some((txid, sl_price)) = self.find_sl_order_for_pair(&kraken_pair).await {
                    log::warn!(
                        "SL recovery: adopted orphaned SL for {}: {:.6} ({})",
                        pair, sl_price, txid
                    );
                    if let Some(trade) = self.open_trades.get_mut(&pair) {
                        trade.stop_loss_order_txid = Some(txid);
                        trade.server_stop_price = sl_price;
                    }
                    self.sl_recovery_last_attempt.remove(&pair);
                    continue;
                }

                // Step 2: nothing to adopt — clear any stale pair orders that
                // may be reserving the asset and causing "Insufficient funds"
                self.cancel_open_orders_for_pair(&kraken_pair).await;
                tokio::time::sleep(std::time::Duration::from_millis(800)).await;

                // Step 3: place fresh SL
                match self.place_stop_loss(&kraken_pair, amount, hard_stop).await {
                    Ok(txid) => {
                        log::warn!(
                            "SL recovery: placed missing SL for {}: {:.6} ({})",
                            pair, hard_stop, txid
                        );
                        if let Some(trade) = self.open_trades.get_mut(&pair) {
                            trade.stop_loss_order_txid = Some(txid);
                            trade.server_stop_price = hard_stop;
                        }
                        self.sl_recovery_last_attempt.remove(&pair);
                    }
                    Err(e) => {
                        // Diagnose: if the asset is held in Earn (opt_in_rewards
                        // auto-allocation), the error will repeat forever until
                        // user manually deallocates. Log it once with details
                        // and apply a longer cooldown (1h) to reduce noise.
                        let asset_symbol = pair.split('/').next().unwrap_or(&pair);
                        if let Some((strategy_id, amount)) = self.check_earn_allocation(asset_symbol).await {
                            log::warn!(
                                "SL recovery {}: failing because asset held in EARN \
                                 (strategy={}, amount={:.6}). Manual deallocation needed via Kraken web UI. \
                                 Backing off recovery to 1h until user resolves.",
                                pair, strategy_id, amount
                            );
                            // Override the 60s throttle with a 1h cooldown
                            self.sl_recovery_last_attempt
                                .insert(pair.clone(), Self::now_sec() + 3540); // +59m beyond throttle
                        } else {
                            log::warn!(
                                "SL recovery: still failing for {}: {} (next retry in {}s)",
                                pair, e, throttle_seconds
                            );
                        }
                    }
                }
                continue;
            }

            let (kraken_pair, amount, new_stop, old_txid) = {
                let trade = match self.open_trades.get(&pair) {
                    Some(t) => t,
                    None => continue,
                };

                let price = match prices.get(&pair) {
                    Some(&p) => p,
                    None => continue,
                };

                let profit = trade.profit_pct(price);
                let atr = trade.entry_atr;
                let profit_in_atr = if atr > 0.0 { profit / atr } else { 0.0 };

                // Progressive trail: tighter as profit grows
                let trail_mult = if profit_in_atr >= 3.0 {
                    0.5
                } else if profit_in_atr >= 2.0 {
                    0.75
                } else if profit_in_atr >= 1.0 {
                    self.config.trail_atr_mult
                } else {
                    0.0
                };

                if trail_mult == 0.0 {
                    // Move to breakeven if peak was 1x ATR above entry
                    let peak_pct = (trade.highest_price - trade.entry_price) / trade.entry_price;
                    let peak_in_atr = if atr > 0.0 { peak_pct / atr } else { 0.0 };
                    if peak_in_atr >= 1.0 && trade.server_stop_price < trade.entry_price * 0.999 {
                        let new_stop = trade.entry_price;
                        (
                            trade.kraken_pair.clone(),
                            trade.amount,
                            new_stop,
                            trade.stop_loss_order_txid.clone(),
                        )
                    } else {
                        continue;
                    }
                } else {
                    // RSI extreme: tighten further
                    let trail_mult = if let Some(&rsi) = rsis.get(&pair) {
                        if rsi > self.config.rsi_overbought {
                            trail_mult.min(0.5)
                        } else {
                            trail_mult
                        }
                    } else {
                        trail_mult
                    };

                    let trail_pct = (atr * trail_mult).clamp(0.01, 0.10);
                    let new_stop = trade.highest_price * (1.0 - trail_pct);
                    let hard_stop = trade.entry_price
                        * (1.0 - (atr * self.config.hard_sl_atr_mult).clamp(0.03, 0.15));
                    let new_stop = new_stop.max(hard_stop);

                    if new_stop <= trade.server_stop_price * 1.005 {
                        continue;
                    }

                    (
                        trade.kraken_pair.clone(),
                        trade.amount,
                        new_stop,
                        trade.stop_loss_order_txid.clone(),
                    )
                }
            };

            if let Some(ref txid) = old_txid {
                match self.edit_stop_loss(txid, &kraken_pair, new_stop).await {
                    Ok(new_txid) => {
                        if let Some(trade) = self.open_trades.get_mut(&pair) {
                            log::info!("SL edit {}: {:.6} → {:.6} ({})", pair, trade.server_stop_price, new_stop, new_txid);
                            trade.stop_loss_order_txid = Some(new_txid);
                            trade.server_stop_price = new_stop;
                        }
                        continue;
                    }
                    Err(e) => {
                        log::warn!("EditOrder failed for {}: {} — falling back to cancel+place", pair, e);
                        let _ = self.cancel_order(txid).await;
                    }
                }
            }

            match self.place_stop_loss(&kraken_pair, amount, new_stop).await {
                Ok(txid) => {
                    if let Some(trade) = self.open_trades.get_mut(&pair) {
                        log::info!("SL placed {}: {:.6} → {:.6} ({})", pair, trade.server_stop_price, new_stop, txid);
                        trade.stop_loss_order_txid = Some(txid);
                        trade.server_stop_price = new_stop;
                    }
                }
                Err(e) => {
                    log::error!("SL update failed for {}: {}", pair, e);
                    if let Some((txid, sl_price)) = self.find_sl_order_for_pair(&kraken_pair).await {
                        log::info!("Re-adopted SL for {}: {:.6} ({})", pair, sl_price, txid);
                        if let Some(trade) = self.open_trades.get_mut(&pair) {
                            trade.stop_loss_order_txid = Some(txid);
                            trade.server_stop_price = sl_price;
                        }
                    } else if let Some(trade) = self.open_trades.get_mut(&pair) {
                        trade.stop_loss_order_txid = None;
                        trade.server_stop_price = 0.0;
                    }
                }
            }
        }
    }

    pub async fn log_status_ws(&self) {
        let open_value: f64 = {
            let mut total = 0.0;
            for t in self.open_trades.values() {
                let price = self.get_ws_price(&t.pair).await.unwrap_or(t.entry_price);
                total += t.amount * price;
            }
            total
        };

        log::info!(
            "=== Status | Cash: €{:.2} | Positions: €{:.2} | Total: €{:.2} | Day P&L: €{:.2} | Open: {}/{} ===",
            self.eur_balance,
            open_value,
            self.eur_balance + open_value,
            self.daily_pnl,
            self.open_trades.len(),
            self.config.max_open_trades,
        );

        for trade in self.open_trades.values() {
            let price = self.get_ws_price(&trade.pair).await.unwrap_or(trade.entry_price);
            let rsi = self.get_ws_rsi(&trade.pair).await.unwrap_or(0.0);
            log::info!(
                "  {} | {:.1}% (€{:.2}) | RSI={:.0} | age={}m | SL={:.6}",
                trade.pair,
                trade.profit_pct(price) * 100.0,
                trade.profit_eur(price),
                rsi,
                trade.age_minutes(),
                trade.server_stop_price,
            );
        }
    }

}
