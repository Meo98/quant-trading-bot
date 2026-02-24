use crate::config::BotConfig;
use crate::trading::OpenTrade;
use crate::api::rest_client::KrakenRestClient;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct TradingEngine {
    pub config: BotConfig,
    pub api: KrakenRestClient,
    pub open_trades: HashMap<String, OpenTrade>,
    pub eur_balance: f64,
    pub all_eur_pairs: Vec<String>,
    pub price_history: HashMap<String, Vec<(u64, f64)>>,
    pub prev_tickers: HashMap<String, f64>,
    pub pump_cooldowns: HashMap<String, u64>,
}

pub struct PumpCandidate {
    pub pair: String,
    pub pct_change: f64,
    pub volume_eur: f64,
    pub price: f64,
    pub volatility: f64,
}

impl TradingEngine {
    pub fn new(config: BotConfig) -> Self {
        let api = KrakenRestClient::new(config.api_key.clone(), config.api_secret.clone());
        Self {
            config,
            api,
            open_trades: HashMap::new(),
            eur_balance: 0.0,
            all_eur_pairs: Vec::new(),
            price_history: HashMap::new(),
            prev_tickers: HashMap::new(),
            pump_cooldowns: HashMap::new(),
        }
    }

    fn now_sec() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    pub async fn detect_pumps(&mut self) -> anyhow::Result<Vec<PumpCandidate>> {
        // Here we would call self.api.public_request("/0/public/Ticker?pair=...")
        // For now, we stub the response parsing to build the logic framework
        
        let mut pumps: Vec<PumpCandidate> = Vec::new();
        let now = Self::now_sec();

        // MOCK LOOP: In reality we iterate over the JSON response from Kraken
        let mock_tickers = vec![
            ("DOGE/EUR", 0.15, 15.0, 50000.0, 0.20), // price, pct_24h, vol_eur, volatility
        ];

        for (symbol, price, pct_24h, vol_eur, volatility) in mock_tickers {
            let pair = symbol.to_string();
            
            // Rolling Price History (2 Hours)
            let history = self.price_history.entry(pair.clone()).or_insert_with(Vec::new);
            history.push((now, price));
            history.retain(|&(t, _)| now - t <= 7200);

            // Basic Filters
            if price < self.config.min_price { continue; }
            if vol_eur < self.config.min_volume_eur { continue; }
            if pct_24h < self.config.min_pct_24h { continue; }
            if pct_24h > 200.0 { continue; } // Max already pumped

            // Cooldown Filter
            if let Some(&cooldown) = self.pump_cooldowns.get(&pair) {
                if now < cooldown { continue; }
            }

            // Trend Filter
            if let Some(&prev_price) = self.prev_tickers.get(&pair) {
                if prev_price > 0.0 && price < prev_price {
                    continue; // Falling knife
                }
            }

            // Short-Term Momentum Filter
            let mut price_15m_ago = price;
            let mut price_1h_ago = price;

            for &(t, p) in history.iter() {
                if now - t <= 900 && price_15m_ago == price { price_15m_ago = p; }
                if now - t <= 3600 && price_1h_ago == price { price_1h_ago = p; }
            }

            let pct_15m = if price_15m_ago > 0.0 { (price - price_15m_ago) / price_15m_ago * 100.0 } else { 0.0 };
            let pct_1h = if price_1h_ago > 0.0 { (price - price_1h_ago) / price_1h_ago * 100.0 } else { 0.0 };

            if pct_15m < -0.5 { continue; }
            if pct_15m < self.config.min_pct_15m && pct_1h < self.config.min_pct_1h { continue; }

            pumps.push(PumpCandidate {
                pair,
                pct_change: pct_24h,
                volume_eur: vol_eur,
                price,
                volatility,
            });
        }

        // Store current prices for next tick
        for p in &pumps {
            self.prev_tickers.insert(p.pair.clone(), p.price);
        }

        // Sort by momentum & volume
        pumps.sort_by(|a, b| {
            let score_a = a.pct_change * a.volume_eur.max(1.0).log10();
            let score_b = b.pct_change * b.volume_eur.max(1.0).log10();
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(pumps)
    }

    pub fn check_exit(&mut self, pair: &str, current_price: f64) -> Option<String> {
        let trade = self.open_trades.get_mut(pair)?;

        let profit = trade.profit_pct(current_price);

        if current_price > trade.highest_price {
            trade.highest_price = current_price;
        }

        if profit <= trade.hard_sl_pct {
            return Some(format!(
                "🛑 STOP-LOSS ({:.1}% / SL: {:.1}%)",
                profit * 100.0,
                trade.hard_sl_pct * 100.0
            ));
        }

        // Dynamic Trailing Stop (Step-Up)
        if profit > 0.015 { // Minimum profit to exit
            let peak_profit = trade.profit_pct(trade.highest_price);
            let mut active_trailing_stop = trade.trailing_stop_pct;

            if peak_profit >= self.config.step_up_2_profit {
                active_trailing_stop = active_trailing_stop.max(self.config.step_up_2_trailing);
            } else if peak_profit >= self.config.step_up_1_profit {
                active_trailing_stop = active_trailing_stop.max(self.config.step_up_1_trailing);
            }

            let drawdown = trade.drawdown_from_high(current_price);
            if drawdown <= -active_trailing_stop {
                let stufe = if active_trailing_stop == self.config.step_up_2_trailing {
                    " [MOON-PHASE 25%]"
                } else if active_trailing_stop == self.config.step_up_1_trailing {
                    " [STEP-UP 15%]"
                } else {
                    ""
                };

                return Some(format!(
                    "📉 TRAILING STOP{} | Peak: {:.6} -> Now: {:.6} ({:.1}% vom High)",
                    stufe,
                    trade.highest_price,
                    current_price,
                    drawdown * 100.0
                ));
            }
        }

        None
    }

    pub async fn execute_buy(&mut self, pair: &str, price: f64, volatility: f64) -> anyhow::Result<bool> {
        if self.open_trades.contains_key(pair) {
            return Ok(false);
        }
        if self.open_trades.len() >= self.config.max_open_trades {
            return Ok(false);
        }

        let remaining_slots = self.config.max_open_trades - self.open_trades.len();
        if remaining_slots == 0 {
            return Ok(false);
        }

        let slot_stake = (self.eur_balance / remaining_slots as f64) * 0.99;
        let stake = slot_stake.min(self.eur_balance * 0.99);

        if stake < 1.0 {
            return Ok(false);
        }

        let amount = stake / price;

        // Phase 2: Dynamic Regime Strategy
        let (mut t_stop, mut h_stop): (f64, f64) = if volatility > 0.40 {
            (0.15, -0.20) // Hyper-volatile memecoin
        } else if volatility > 0.15 {
            (0.10, -0.15) // Volatile Meme
        } else {
            (0.06, -0.10) // Calm Altcoin
        };

        t_stop = t_stop.max(self.config.trailing_stop_pct);
        h_stop = h_stop.min(self.config.hard_sl_pct);

        println!("🟢 MOCK BUY {} | €{:.2} @ {:.6} | Vol: {:.1}%", pair, stake, price, volatility * 100.0);

        self.open_trades.insert(
            pair.to_string(),
            OpenTrade {
                pair: pair.to_string(),
                entry_price: price,
                amount,
                stake_eur: stake,
                highest_price: price,
                trailing_stop_pct: t_stop,
                hard_sl_pct: h_stop,
            },
        );

        self.eur_balance -= stake;

        Ok(true)
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        println!("🚀 Rust Momentum Engine Started!");
        Ok(())
    }
}
