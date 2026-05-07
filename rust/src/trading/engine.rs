use crate::api::rest_client::KrakenRestClient;
use crate::config::BotConfig;
use crate::trading::indicators::{Indicators, PriceBar};
use crate::trading::OpenTrade;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

const RSI_PERIOD: usize = 30;
const EMA_SHORT: usize = 20;
const EMA_LONG: usize = 50;
const ATR_PERIOD: usize = 20;
const VOL_AVG_PERIOD: usize = 30;
const MIN_BARS_FOR_SIGNALS: usize = 55;
const BTC_PAIR: &str = "BTC/EUR";

#[derive(Debug)]
pub struct EntrySignal {
    pub pair: String,
    pub kraken_pair: String,
    pub price: f64,
    pub rsi: f64,
    pub atr_pct: f64,
    pub volume_ratio: f64,
    pub score: f64,
}

pub struct TradingEngine {
    pub config: BotConfig,
    pub api: KrakenRestClient,
    pub open_trades: HashMap<String, OpenTrade>,
    pub eur_balance: f64,
    pub all_eur_pairs: HashMap<String, String>,
    pub indicators: HashMap<String, Indicators>,
    pub liquid_pairs: Vec<(String, String, f64)>,
    pub cooldowns: HashMap<String, u64>,
    pub daily_start_balance: f64,
    pub daily_pnl: f64,
    pub last_daily_reset: u64,
    pub tick_count: u64,
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
            indicators: HashMap::new(),
            liquid_pairs: Vec::new(),
            cooldowns: HashMap::new(),
            daily_start_balance: 0.0,
            daily_pnl: 0.0,
            last_daily_reset: 0,
            tick_count: 0,
        }
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
        self.daily_start_balance = self.eur_balance;
        self.last_daily_reset = Self::now_sec();
        log::info!(
            "Engine ready: {} pairs, €{:.2} balance",
            self.all_eur_pairs.len(),
            self.eur_balance
        );
        Ok(())
    }

    async fn fetch_eur_pairs(&mut self) -> Result<()> {
        let result = self.api.public_request("/0/public/AssetPairs", &[]).await?;
        if let Value::Object(pairs) = result {
            self.all_eur_pairs.clear();
            for (kraken_pair, info) in pairs {
                if let Some(quote) = info.get("quote").and_then(|q| q.as_str()) {
                    if (quote == "ZEUR" || quote == "EUR") && !kraken_pair.contains(".d") {
                        let wsname = info
                            .get("wsname")
                            .and_then(|w| w.as_str())
                            .unwrap_or(&kraken_pair);
                        self.all_eur_pairs
                            .insert(wsname.to_string(), kraken_pair.clone());
                    }
                }
            }
            log::info!("Loaded {} EUR pairs", self.all_eur_pairs.len());
        }
        Ok(())
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
        }
        Ok(())
    }

    pub fn reset_daily_if_needed(&mut self) {
        let now = Self::now_sec();
        if now - self.last_daily_reset >= 86400 {
            let total_balance = self.eur_balance
                + self.open_trades.values().map(|t| t.stake_eur).sum::<f64>();
            log::info!(
                "Daily reset | Previous: €{:.2} → Now: €{:.2} | Day P&L: {:.2}",
                self.daily_start_balance,
                total_balance,
                self.daily_pnl
            );
            self.daily_start_balance = total_balance.max(self.eur_balance);
            self.daily_pnl = 0.0;
            self.last_daily_reset = now;
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
                .or_insert_with(|| Indicators::new(500));
            ind.push(PriceBar {
                timestamp: now,
                price,
                volume_eur,
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

    /// Scan liquid pairs for entry signals (RSI oversold bounce)
    pub fn scan_entries(&self) -> Vec<EntrySignal> {
        let mut signals = Vec::new();

        if !self.is_daily_drawdown_ok() {
            return signals;
        }

        if self.open_trades.len() >= self.config.max_open_trades {
            return signals;
        }

        // BTC market filter
        if let Some(btc_ind) = self.indicators.get(BTC_PAIR) {
            if let Some(btc_rsi) = btc_ind.rsi(RSI_PERIOD) {
                if btc_rsi < 20.0 {
                    log::info!("Market filter: BTC RSI={:.1} (too low), skipping entries", btc_rsi);
                    return signals;
                }
            }
        }

        let now = Self::now_sec();

        for (display, kraken, _vol) in &self.liquid_pairs {
            if self.open_trades.contains_key(display) {
                continue;
            }
            if let Some(&cd) = self.cooldowns.get(display) {
                if now < cd {
                    continue;
                }
            }

            let ind = match self.indicators.get(display) {
                Some(i) if i.len() >= MIN_BARS_FOR_SIGNALS => i,
                _ => continue,
            };

            let rsi = match ind.rsi(RSI_PERIOD) {
                Some(r) => r,
                None => continue,
            };
            let rsi_prev = match ind.rsi_prev(RSI_PERIOD) {
                Some(r) => r,
                None => continue,
            };
            let price = match ind.last_price() {
                Some(p) => p,
                None => continue,
            };
            let ema_short = match ind.ema(EMA_SHORT) {
                Some(e) => e,
                None => continue,
            };
            let atr_pct = match ind.atr_pct(ATR_PERIOD) {
                Some(a) => a,
                None => continue,
            };

            // Volume confirmation
            let vol_ratio = match (ind.current_volume(), ind.avg_volume(VOL_AVG_PERIOD)) {
                (Some(cur), Some(avg)) if avg > 0.0 => cur / avg,
                _ => 1.0,
            };

            // === ENTRY CONDITIONS ===
            // 1. RSI crossed up from oversold zone (<30 → >30)
            let rsi_cross_up = rsi_prev < self.config.rsi_oversold && rsi >= self.config.rsi_oversold;

            // 2. Or RSI is in recovery zone (30-40) AND price above short EMA (trend confirmation)
            let rsi_recovery = rsi > self.config.rsi_oversold
                && rsi < 40.0
                && price > ema_short;

            if !rsi_cross_up && !rsi_recovery {
                continue;
            }

            // 3. Volume at least average (no dead-cat bounces on zero volume)
            if vol_ratio < 0.8 {
                continue;
            }

            // 4. ATR sanity: skip extremely volatile or dead pairs
            if atr_pct < 0.001 || atr_pct > 0.15 {
                continue;
            }

            let score = (40.0 - rsi) * vol_ratio * (1.0 / atr_pct.max(0.01));

            signals.push(EntrySignal {
                pair: display.clone(),
                kraken_pair: kraken.clone(),
                price,
                rsi,
                atr_pct,
                volume_ratio: vol_ratio,
                score,
            });
        }

        signals.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        signals
    }

    /// Check all open trades for exit conditions
    pub fn scan_exits(&mut self) -> Vec<(String, String)> {
        let mut exits = Vec::new();

        let pairs: Vec<String> = self.open_trades.keys().cloned().collect();
        for pair in pairs {
            let trade = match self.open_trades.get_mut(&pair) {
                Some(t) => t,
                None => continue,
            };

            let ind = match self.indicators.get(&pair) {
                Some(i) => i,
                None => continue,
            };

            let price = match ind.last_price() {
                Some(p) => p,
                None => continue,
            };

            if price > trade.highest_price {
                trade.highest_price = price;
            }

            let profit = trade.profit_pct(price);
            let profit_eur = trade.profit_eur(price);
            let age = trade.age_minutes();

            // 1. Hard stop: entry price - 2.5x ATR
            let hard_sl_pct = -(trade.entry_atr * self.config.hard_sl_atr_mult).clamp(0.03, 0.20);
            if profit <= hard_sl_pct {
                exits.push((
                    pair.clone(),
                    format!(
                        "HARD-SL: {:.1}% (limit {:.1}%) | €{:.2}",
                        profit * 100.0,
                        hard_sl_pct * 100.0,
                        profit_eur
                    ),
                ));
                continue;
            }

            // 2. Trailing stop: 2x ATR from peak (only after 1.5% profit)
            if profit > 0.015 {
                let trail_pct = (trade.entry_atr * self.config.trail_atr_mult).clamp(0.02, 0.12);
                let drawdown = trade.drawdown_from_high(price);
                if drawdown <= -trail_pct {
                    exits.push((
                        pair.clone(),
                        format!(
                            "TRAIL-STOP: {:.1}% from peak (trail {:.1}%) | P/L: {:.1}% €{:.2}",
                            drawdown * 100.0,
                            trail_pct * 100.0,
                            profit * 100.0,
                            profit_eur
                        ),
                    ));
                    continue;
                }
            }

            // 3. RSI overbought take-profit
            if let Some(rsi) = ind.rsi(RSI_PERIOD) {
                if rsi > self.config.rsi_overbought && profit > 0.01 {
                    exits.push((
                        pair.clone(),
                        format!(
                            "RSI-TP: RSI={:.1} | P/L: {:.1}% €{:.2}",
                            rsi,
                            profit * 100.0,
                            profit_eur
                        ),
                    ));
                    continue;
                }
            }

            // 4. Time stop: after max_hold_hours with no meaningful profit
            if age > self.config.max_hold_minutes && profit < 0.02 {
                exits.push((
                    pair.clone(),
                    format!(
                        "TIME-STOP: {}h {}m | P/L: {:.1}% €{:.2}",
                        age / 60,
                        age % 60,
                        profit * 100.0,
                        profit_eur
                    ),
                ));
            }
        }

        exits
    }

    pub async fn execute_buy(&mut self, signal: &EntrySignal) -> Result<bool> {
        if self.open_trades.contains_key(&signal.pair) {
            return Ok(false);
        }
        if self.open_trades.len() >= self.config.max_open_trades {
            return Ok(false);
        }

        let remaining = self.config.max_open_trades - self.open_trades.len();
        let stake = (self.eur_balance * 0.45 / remaining as f64).min(self.eur_balance * 0.95);
        if stake < 5.0 {
            log::warn!("Skip buy {}: stake €{:.2} too small", signal.pair, stake);
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
                    "BUY {} | €{:.2} @ {:.6} | RSI={:.1} ATR={:.2}% VolR={:.1}x | {:?}",
                    signal.pair,
                    stake,
                    signal.price,
                    signal.rsi,
                    signal.atr_pct * 100.0,
                    signal.volume_ratio,
                    resp
                );

                let hard_sl_price =
                    signal.price * (1.0 - (signal.atr_pct * self.config.hard_sl_atr_mult).clamp(0.03, 0.20));

                let mut trade = OpenTrade {
                    pair: signal.pair.clone(),
                    kraken_pair: signal.kraken_pair.clone(),
                    entry_price: signal.price,
                    amount,
                    stake_eur: stake,
                    highest_price: signal.price,
                    entry_time: Self::now_sec(),
                    stop_loss_order_txid: None,
                    server_stop_price: 0.0,
                    entry_atr: signal.atr_pct,
                    exit_reason: None,
                };

                match self
                    .place_stop_loss(&signal.kraken_pair, amount, hard_sl_price)
                    .await
                {
                    Ok(txid) => {
                        log::info!(
                            "Server SL for {}: {:.6} (txid={})",
                            signal.pair,
                            hard_sl_price,
                            txid
                        );
                        trade.stop_loss_order_txid = Some(txid);
                        trade.server_stop_price = hard_sl_price;
                    }
                    Err(e) => log::error!("SL placement failed for {}: {}", signal.pair, e),
                }

                self.open_trades.insert(signal.pair.clone(), trade);
                self.eur_balance -= stake;
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

    pub async fn execute_sell(&mut self, pair: &str, reason: &str) -> Result<bool> {
        let trade = match self.open_trades.get(pair) {
            Some(t) => t.clone(),
            None => return Ok(false),
        };

        if let Some(ref txid) = trade.stop_loss_order_txid {
            match self.cancel_order(txid).await {
                Ok(_) => log::info!("Cancelled SL {} for {}", txid, pair),
                Err(e) => log::warn!("SL cancel failed for {}: {}", pair, e),
            }
        }

        let result = self
            .api
            .private_request(
                "/0/private/AddOrder",
                vec![
                    ("pair", trade.kraken_pair.clone()),
                    ("type", "sell".to_string()),
                    ("ordertype", "market".to_string()),
                    ("volume", format!("{:.8}", trade.amount)),
                ],
            )
            .await;

        match result {
            Ok(resp) => {
                let pnl = trade.profit_eur(
                    self.indicators
                        .get(pair)
                        .and_then(|i| i.last_price())
                        .unwrap_or(trade.entry_price),
                );
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
                log::error!("Sell failed {}: {}", pair, e);
                Err(e)
            }
        }
    }

    pub async fn update_trailing_stops(&mut self) {
        let pairs: Vec<String> = self.open_trades.keys().cloned().collect();

        for pair in pairs {
            let (kraken_pair, amount, new_stop, old_txid) = {
                let trade = match self.open_trades.get(&pair) {
                    Some(t) => t,
                    None => continue,
                };

                let price = match self.indicators.get(&pair).and_then(|i| i.last_price()) {
                    Some(p) => p,
                    None => continue,
                };

                let profit = trade.profit_pct(price);
                if profit <= 0.015 {
                    continue;
                }

                let trail_pct = (trade.entry_atr * self.config.trail_atr_mult).clamp(0.02, 0.12);
                let new_stop = trade.highest_price * (1.0 - trail_pct);
                let hard_stop =
                    trade.entry_price * (1.0 - (trade.entry_atr * self.config.hard_sl_atr_mult).clamp(0.03, 0.20));
                let new_stop = new_stop.max(hard_stop);

                if new_stop <= trade.server_stop_price * 1.01 {
                    continue;
                }

                (
                    trade.kraken_pair.clone(),
                    trade.amount,
                    new_stop,
                    trade.stop_loss_order_txid.clone(),
                )
            };

            if let Some(ref txid) = old_txid {
                if let Err(e) = self.cancel_order(txid).await {
                    log::warn!("Cancel old SL for {}: {}", pair, e);
                    continue;
                }
            }

            match self.place_stop_loss(&kraken_pair, amount, new_stop).await {
                Ok(txid) => {
                    if let Some(trade) = self.open_trades.get_mut(&pair) {
                        log::info!(
                            "SL update {}: {:.6} → {:.6} ({})",
                            pair,
                            trade.server_stop_price,
                            new_stop,
                            txid
                        );
                        trade.stop_loss_order_txid = Some(txid);
                        trade.server_stop_price = new_stop;
                    }
                }
                Err(e) => {
                    log::error!("SL update failed for {}: {}", pair, e);
                    if let Some(trade) = self.open_trades.get_mut(&pair) {
                        trade.stop_loss_order_txid = None;
                        trade.server_stop_price = 0.0;
                    }
                }
            }
        }
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
                    ("price", format!("{:.8}", stop_price)),
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

    async fn cancel_order(&self, txid: &str) -> Result<()> {
        self.api
            .private_request("/0/private/CancelOrder", vec![("txid", txid.to_string())])
            .await?;
        Ok(())
    }

    pub fn log_status(&self) {
        self.tick_count;
        let open_value: f64 = self
            .open_trades
            .values()
            .map(|t| {
                let price = self
                    .indicators
                    .get(&t.pair)
                    .and_then(|i| i.last_price())
                    .unwrap_or(t.entry_price);
                t.amount * price
            })
            .sum();

        log::info!(
            "=== Tick #{} | Cash: €{:.2} | Positions: €{:.2} | Total: €{:.2} | Day P&L: €{:.2} | Open: {}/{} ===",
            self.tick_count,
            self.eur_balance,
            open_value,
            self.eur_balance + open_value,
            self.daily_pnl,
            self.open_trades.len(),
            self.config.max_open_trades,
        );

        for trade in self.open_trades.values() {
            let price = self
                .indicators
                .get(&trade.pair)
                .and_then(|i| i.last_price())
                .unwrap_or(trade.entry_price);
            let rsi = self
                .indicators
                .get(&trade.pair)
                .and_then(|i| i.rsi(RSI_PERIOD))
                .unwrap_or(0.0);
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

        if self.liquid_pairs.len() > 0 {
            let sample: Vec<String> = self
                .liquid_pairs
                .iter()
                .take(5)
                .filter_map(|(name, _, _)| {
                    let ind = self.indicators.get(name)?;
                    let rsi = ind.rsi(RSI_PERIOD)?;
                    Some(format!("{}={:.0}", name.replace("/EUR", ""), rsi))
                })
                .collect();
            log::info!("  Top RSI: {}", sample.join(" | "));
        }
    }
}
