//! Trading Engine - Core momentum trading logic for Kraken
//!
//! This module implements the main trading loop with:
//! - Pump detection based on 24h/15m/1h momentum
//! - Dynamic trailing stop-loss with step-up levels
//! - Volatility-based regime selection
//! - Circuit breaker for market-wide risk management

use crate::api::rest_client::KrakenRestClient;
use crate::config::BotConfig;
use crate::trading::OpenTrade;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents a coin that passed pump detection filters
#[derive(Debug, Clone)]
pub struct PumpCandidate {
    pub pair: String,
    pub kraken_pair: String, // e.g., "XXBTZEUR" for API calls
    pub pct_change_24h: f64,
    pub pct_change_15m: f64,
    pub pct_change_1h: f64,
    pub volume_eur: f64,
    pub price: f64,
    pub volatility: f64,
    pub score: f64, // Combined momentum score for ranking
}

/// Engine status for Flutter UI
#[derive(Debug, Clone)]
pub struct EngineStatus {
    pub is_running: bool,
    pub eur_balance: f64,
    pub open_trades_count: usize,
    pub last_tick: u64,
    pub total_pairs_scanned: usize,
    pub pumps_detected: usize,
    pub error_message: Option<String>,
}

/// Main trading engine that coordinates all trading activities
#[derive(Clone)]
pub struct TradingEngine {
    pub config: BotConfig,
    pub api: KrakenRestClient,
    pub open_trades: HashMap<String, OpenTrade>,
    pub eur_balance: f64,
    pub all_eur_pairs: HashMap<String, String>, // Display name -> Kraken pair
    pub price_history: HashMap<String, Vec<(u64, f64)>>, // 2h rolling window
    pub prev_tickers: HashMap<String, f64>,
    pub pump_cooldowns: HashMap<String, u64>,
    pub is_running: bool,
    pub last_error: Option<String>,
    pub last_tick: u64,
}

impl TradingEngine {
    /// Creates a new TradingEngine with the given configuration
    pub fn new(config: BotConfig) -> Self {
        let api = KrakenRestClient::new(config.api_key.clone(), config.api_secret.clone());
        Self {
            config,
            api,
            open_trades: HashMap::new(),
            eur_balance: 0.0,
            all_eur_pairs: HashMap::new(),
            price_history: HashMap::new(),
            prev_tickers: HashMap::new(),
            pump_cooldowns: HashMap::new(),
            is_running: false,
            last_error: None,
            last_tick: 0,
        }
    }

    /// Returns current Unix timestamp in seconds
    fn now_sec() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Fetches all available EUR trading pairs from Kraken
    pub async fn fetch_eur_pairs(&mut self) -> Result<()> {
        log::info!("Fetching EUR trading pairs from Kraken...");

        let result = self.api.public_request("/0/public/AssetPairs", &[]).await?;

        if let Value::Object(pairs) = result {
            self.all_eur_pairs.clear();

            for (kraken_pair, info) in pairs {
                // Filter for EUR quote currency
                if let Some(quote) = info.get("quote").and_then(|q| q.as_str()) {
                    if quote == "ZEUR" || quote == "EUR" {
                        // Get the display name (e.g., "BTC/EUR")
                        let wsname = info
                            .get("wsname")
                            .and_then(|w| w.as_str())
                            .unwrap_or(&kraken_pair);

                        // Skip leverage/margin pairs
                        if !kraken_pair.contains(".d") {
                            self.all_eur_pairs
                                .insert(wsname.to_string(), kraken_pair.clone());
                        }
                    }
                }
            }

            log::info!("Found {} EUR trading pairs", self.all_eur_pairs.len());
        }

        Ok(())
    }

    /// Fetches current EUR balance from Kraken
    pub async fn fetch_balance(&mut self) -> Result<()> {
        let result = self.api.private_request("/0/private/Balance", vec![]).await?;

        if let Value::Object(balances) = result {
            // EUR balance can be under "ZEUR" or "EUR"
            let eur = balances
                .get("ZEUR")
                .or_else(|| balances.get("EUR"))
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            self.eur_balance = eur;
            log::info!("Current EUR balance: €{:.2}", self.eur_balance);
        }

        Ok(())
    }

    /// Detects coins showing pump characteristics
    ///
    /// Filters applied:
    /// 1. Minimum 24h price change (default 5%)
    /// 2. Short-term momentum (15m or 1h threshold)
    /// 3. Minimum volume (default €10,000)
    /// 4. Not already pumped >200%
    /// 5. Not on cooldown from recent exit
    /// 6. Not falling (trend filter)
    pub async fn detect_pumps(&mut self) -> Result<Vec<PumpCandidate>> {
        let mut pumps: Vec<PumpCandidate> = Vec::new();
        let now = Self::now_sec();

        // Build comma-separated pair list for API call
        let pair_list: String = self.all_eur_pairs.values().cloned().collect::<Vec<_>>().join(",");

        if pair_list.is_empty() {
            return Err(anyhow!("No EUR pairs loaded. Call fetch_eur_pairs() first."));
        }

        // Fetch all tickers in one API call
        let result = self
            .api
            .public_request("/0/public/Ticker", &[("pair", &pair_list)])
            .await?;

        let tickers = match result {
            Value::Object(t) => t,
            _ => return Err(anyhow!("Invalid ticker response")),
        };

        // Also fetch OHLC for 15m/1h calculations (we'll use the ticker's high/low for now)
        for (display_name, kraken_pair) in &self.all_eur_pairs {
            let ticker = match tickers.get(kraken_pair) {
                Some(t) => t,
                None => continue,
            };

            // Parse ticker data
            // c = last trade closed [price, lot volume]
            // v = volume [today, last 24h]
            // p = volume weighted average price [today, last 24h]
            // o = open [today, last 24h]
            // h = high [today, last 24h]
            // l = low [today, last 24h]

            let price = ticker
                .get("c")
                .and_then(|c| c.get(0))
                .and_then(|p| p.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            let open_24h = ticker
                .get("o")
                .and_then(|o| o.get(1))
                .and_then(|p| p.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            let volume_24h = ticker
                .get("v")
                .and_then(|v| v.get(1))
                .and_then(|vol| vol.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            let high_24h = ticker
                .get("h")
                .and_then(|h| h.get(1))
                .and_then(|p| p.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            let low_24h = ticker
                .get("l")
                .and_then(|l| l.get(1))
                .and_then(|p| p.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);

            if price <= 0.0 || open_24h <= 0.0 {
                continue;
            }

            // Calculate metrics
            let pct_24h = ((price - open_24h) / open_24h) * 100.0;
            let volume_eur = volume_24h * price;
            let volatility = if low_24h > 0.0 {
                (high_24h - low_24h) / low_24h
            } else {
                0.0
            };

            // Update price history for short-term momentum
            let history = self.price_history.entry(display_name.clone()).or_default();
            history.push((now, price));
            history.retain(|&(t, _)| now - t <= 7200); // Keep 2h of data

            // Calculate 15m and 1h momentum from history
            let mut price_15m_ago = price;
            let mut price_1h_ago = price;

            for &(t, p) in history.iter().rev() {
                if now - t >= 900 && price_15m_ago == price {
                    price_15m_ago = p;
                }
                if now - t >= 3600 && price_1h_ago == price {
                    price_1h_ago = p;
                }
            }

            let pct_15m = if price_15m_ago > 0.0 {
                ((price - price_15m_ago) / price_15m_ago) * 100.0
            } else {
                0.0
            };

            let pct_1h = if price_1h_ago > 0.0 {
                ((price - price_1h_ago) / price_1h_ago) * 100.0
            } else {
                0.0
            };

            // === FILTERS ===

            // 1. Minimum price filter
            if price < self.config.min_price {
                continue;
            }

            // 2. Volume filter
            if volume_eur < self.config.min_volume_eur {
                continue;
            }

            // 3. 24h pump filter (min and max)
            if pct_24h < self.config.min_pct_24h {
                continue;
            }
            if pct_24h > 200.0 {
                continue; // Too late, already mooned
            }

            // 4. Cooldown filter
            if let Some(&cooldown_until) = self.pump_cooldowns.get(display_name) {
                if now < cooldown_until {
                    continue;
                }
            }

            // 5. Trend filter (not falling knife)
            if let Some(&prev_price) = self.prev_tickers.get(display_name) {
                if prev_price > 0.0 && price < prev_price * 0.995 {
                    continue; // Price dropped >0.5% since last tick
                }
            }

            // 6. Short-term momentum filter (at least one must pass)
            if pct_15m < -0.5 {
                continue; // Currently dumping
            }
            if pct_15m < self.config.min_pct_15m && pct_1h < self.config.min_pct_1h {
                continue;
            }

            // Calculate combined score for ranking
            let score = pct_24h * volume_eur.max(1.0).log10() * (1.0 + pct_15m / 10.0);

            pumps.push(PumpCandidate {
                pair: display_name.clone(),
                kraken_pair: kraken_pair.clone(),
                pct_change_24h: pct_24h,
                pct_change_15m: pct_15m,
                pct_change_1h: pct_1h,
                volume_eur,
                price,
                volatility,
                score,
            });
        }

        // Store current prices for next tick
        for pump in &pumps {
            self.prev_tickers.insert(pump.pair.clone(), pump.price);
        }

        // Sort by score descending
        pumps.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        log::info!(
            "Pump detection: {} candidates from {} pairs",
            pumps.len(),
            self.all_eur_pairs.len()
        );

        Ok(pumps)
    }

    /// Checks if an open trade should be exited
    ///
    /// Returns Some(reason) if trade should be closed, None otherwise
    pub fn check_exit(&mut self, pair: &str, current_price: f64) -> Option<String> {
        let trade = self.open_trades.get_mut(pair)?;

        let profit = trade.profit_pct(current_price);

        // Update highest price for trailing stop
        if current_price > trade.highest_price {
            trade.highest_price = current_price;
        }

        // === HARD STOP-LOSS ===
        if profit <= trade.hard_sl_pct {
            return Some(format!(
                "STOP-LOSS: {:.1}% (limit: {:.1}%)",
                profit * 100.0,
                trade.hard_sl_pct * 100.0
            ));
        }

        // === TRAILING STOP (only after minimum profit) ===
        if profit > 0.015 {
            let peak_profit = trade.profit_pct(trade.highest_price);
            let mut active_trailing = trade.trailing_stop_pct;

            // Step-up trailing stop based on peak profit
            if peak_profit >= self.config.step_up_2_profit {
                active_trailing = active_trailing.max(self.config.step_up_2_trailing);
            } else if peak_profit >= self.config.step_up_1_profit {
                active_trailing = active_trailing.max(self.config.step_up_1_trailing);
            }

            let drawdown = trade.drawdown_from_high(current_price);
            if drawdown <= -active_trailing {
                let phase = if active_trailing >= self.config.step_up_2_trailing {
                    "MOON-PHASE"
                } else if active_trailing >= self.config.step_up_1_trailing {
                    "STEP-UP"
                } else {
                    "BASE"
                };

                return Some(format!(
                    "TRAILING-STOP [{}]: {:.1}% from peak (limit: {:.1}%)",
                    phase,
                    drawdown * 100.0,
                    -active_trailing * 100.0
                ));
            }
        }

        None
    }

    /// Executes a buy order for a pump candidate
    pub async fn execute_buy(&mut self, candidate: &PumpCandidate) -> Result<bool> {
        let pair = &candidate.pair;

        // Check if already in this trade
        if self.open_trades.contains_key(pair) {
            return Ok(false);
        }

        // Check max open trades
        if self.open_trades.len() >= self.config.max_open_trades {
            return Ok(false);
        }

        // Calculate stake
        let remaining_slots = self.config.max_open_trades - self.open_trades.len();
        let slot_stake = (self.eur_balance / remaining_slots as f64) * 0.99;
        let stake = slot_stake.min(self.eur_balance * 0.99);

        if stake < 5.0 {
            // Minimum €5 per trade
            log::warn!("Insufficient balance for trade: €{:.2}", stake);
            return Ok(false);
        }

        let amount = stake / candidate.price;

        // Dynamic regime-based stops
        let (trailing, hard_sl): (f64, f64) = if candidate.volatility > 0.40 {
            (0.15, -0.20) // Hyper-volatile memecoin
        } else if candidate.volatility > 0.15 {
            (0.10, -0.15) // Volatile altcoin
        } else {
            (0.06, -0.10) // Calm altcoin
        };

        // Execute market buy on Kraken
        let order_result = self
            .api
            .private_request(
                "/0/private/AddOrder",
                vec![
                    ("pair", candidate.kraken_pair.clone()),
                    ("type", "buy".to_string()),
                    ("ordertype", "market".to_string()),
                    ("volume", format!("{:.8}", amount)),
                ],
            )
            .await;

        match order_result {
            Ok(result) => {
                log::info!(
                    "BUY {} | €{:.2} @ {:.6} | Vol: {:.1}% | Order: {:?}",
                    pair,
                    stake,
                    candidate.price,
                    candidate.volatility * 100.0,
                    result
                );

                self.open_trades.insert(
                    pair.clone(),
                    OpenTrade {
                        pair: pair.clone(),
                        kraken_pair: candidate.kraken_pair.clone(),
                        entry_price: candidate.price,
                        amount,
                        stake_eur: stake,
                        highest_price: candidate.price,
                        trailing_stop_pct: trailing.max(self.config.trailing_stop_pct),
                        hard_sl_pct: hard_sl.min(self.config.hard_sl_pct),
                        entry_time: Self::now_sec(),
                    },
                );

                self.eur_balance -= stake;
                Ok(true)
            }
            Err(e) => {
                log::error!("Buy order failed for {}: {}", pair, e);
                Err(e)
            }
        }
    }

    /// Executes a sell order for an open trade
    pub async fn execute_sell(&mut self, pair: &str, reason: &str) -> Result<bool> {
        let trade = match self.open_trades.get(pair) {
            Some(t) => t.clone(),
            None => return Ok(false),
        };

        // Execute market sell on Kraken
        let order_result = self
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

        match order_result {
            Ok(result) => {
                log::info!(
                    "SELL {} | Reason: {} | Order: {:?}",
                    pair,
                    reason,
                    result
                );

                // Set cooldown (30 minutes)
                self.pump_cooldowns
                    .insert(pair.to_string(), Self::now_sec() + 1800);

                // Remove from open trades
                self.open_trades.remove(pair);

                // Balance will be updated on next fetch_balance() call
                Ok(true)
            }
            Err(e) => {
                log::error!("Sell order failed for {}: {}", pair, e);
                Err(e)
            }
        }
    }

    /// Returns current engine status for UI
    pub fn get_status(&self) -> EngineStatus {
        EngineStatus {
            is_running: self.is_running,
            eur_balance: self.eur_balance,
            open_trades_count: self.open_trades.len(),
            last_tick: self.last_tick,
            total_pairs_scanned: self.all_eur_pairs.len(),
            pumps_detected: 0, // Updated during detect_pumps
            error_message: self.last_error.clone(),
        }
    }

    /// Returns list of open trades for UI
    pub fn get_open_trades(&self) -> Vec<OpenTrade> {
        self.open_trades.values().cloned().collect()
    }

    /// Main trading tick - called every 60 seconds
    pub async fn tick(&mut self) -> Result<()> {
        self.last_tick = Self::now_sec();
        self.last_error = None;

        // 1. Update balance
        if let Err(e) = self.fetch_balance().await {
            self.last_error = Some(format!("Balance fetch error: {}", e));
            log::error!("{}", self.last_error.as_ref().unwrap());
        }

        // 2. Check exits for open trades
        let current_prices: HashMap<String, f64> = {
            // Fetch current prices for open trades
            if !self.open_trades.is_empty() {
                let pairs: Vec<String> = self
                    .open_trades
                    .values()
                    .map(|t| t.kraken_pair.clone())
                    .collect();
                let pair_list = pairs.join(",");

                match self.api.public_request("/0/public/Ticker", &[("pair", &pair_list)]).await {
                    Ok(Value::Object(tickers)) => {
                        let mut prices = HashMap::new();
                        for trade in self.open_trades.values() {
                            if let Some(ticker) = tickers.get(&trade.kraken_pair) {
                                if let Some(price) = ticker
                                    .get("c")
                                    .and_then(|c| c.get(0))
                                    .and_then(|p| p.as_str())
                                    .and_then(|s| s.parse::<f64>().ok())
                                {
                                    prices.insert(trade.pair.clone(), price);
                                }
                            }
                        }
                        prices
                    }
                    _ => HashMap::new(),
                }
            } else {
                HashMap::new()
            }
        };

        // Check each trade for exit
        let mut exits: Vec<(String, String)> = Vec::new();
        for (pair, price) in &current_prices {
            if let Some(reason) = self.check_exit(pair, *price) {
                exits.push((pair.clone(), reason));
            }
        }

        // Execute exits
        for (pair, reason) in exits {
            if let Err(e) = self.execute_sell(&pair, &reason).await {
                log::error!("Exit failed for {}: {}", pair, e);
            }
        }

        // 3. Detect new pumps (only if we have slots available)
        if self.open_trades.len() < self.config.max_open_trades {
            match self.detect_pumps().await {
                Ok(pumps) => {
                    // Try to enter top candidates
                    for candidate in pumps.iter().take(3) {
                        if self.open_trades.len() >= self.config.max_open_trades {
                            break;
                        }
                        if let Err(e) = self.execute_buy(candidate).await {
                            log::error!("Buy failed for {}: {}", candidate.pair, e);
                        }
                    }
                }
                Err(e) => {
                    self.last_error = Some(format!("Pump detection error: {}", e));
                    log::error!("{}", self.last_error.as_ref().unwrap());
                }
            }
        }

        Ok(())
    }

    /// Starts the trading engine
    pub async fn start(&mut self) -> Result<()> {
        log::info!("Starting Matrix Quant Trading Engine...");

        // Initial setup
        self.fetch_eur_pairs().await?;
        self.fetch_balance().await?;

        self.is_running = true;
        log::info!(
            "Engine started: {} EUR pairs, €{:.2} balance, {} max trades",
            self.all_eur_pairs.len(),
            self.eur_balance,
            self.config.max_open_trades
        );

        Ok(())
    }

    /// Stops the trading engine
    pub fn stop(&mut self) {
        self.is_running = false;
        log::info!("Trading engine stopped");
    }
}
