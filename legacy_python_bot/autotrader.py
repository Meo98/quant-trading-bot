#!/usr/bin/env python3
"""
Kraken Momentum Autotrader – Pump-Rider
Erkennt Coins die GERADE pumpen und reitet die Welle mit Trailing Stop.
Scannt alle 582 EUR-Pairs in ~5 Sekunden via Bulk-Ticker.
"""

import ccxt
import json
import time
import logging
import sys
import urllib.request
import urllib.parse
from datetime import datetime, timezone
from dataclasses import dataclass, field
from pathlib import Path

# ══════════════════════════════════════════════════════════════════════════════
#  KONFIGURATION
# ══════════════════════════════════════════════════════════════════════════════

DRY_RUN = False               # Wenn True: Keine echten Orders auf Kraken (nur Ledger/Log)
CONFIG_FILE = Path(__file__).parent / "config.json"

# Timing & Sizing
CHECK_INTERVAL = 5        # Sekunden zwischen Zyklen (sehr schnell!)
STAKE_CURRENCY = "EUR"
MAX_OPEN_TRADES = 3       # Wie viele Coins dürfen parallel gehandelt werden?

# ── Pump-Erkennung ──────────────────────────────────────────────────────────
# Ein Coin wird als "Pump" erkannt wenn er diese Kriterien erfüllt:
PUMP_MIN_PCT_24H = 5.0        # Mind. +5% in 24h
PUMP_MIN_PCT_15M = 1.0        # Mind. +1.0% in letzten 15 Minuten (vermeidet alte Pumps)
PUMP_MIN_PCT_1H  = 2.0        # ODER Alternativ: +2.0% in letzter 1 Stunde
PUMP_MIN_VOLUME_EUR = 10000   # Mind. €10k 24h-Volumen (filtert Scam-Coins)
PUMP_MIN_PRICE = 0.0001       # Mindestpreis in EUR
PUMP_MAX_ALREADY_PUMPED = 200 # Ignoriere Coins die schon >200% gestiegen sind (zu spät)

# ── Trailing Stop (das Herzstück!) ──────────────────────────────────────────
TRAILING_STOP_PCT = 0.10      # Verkaufe wenn Preis 10% vom Höchststand fällt (Memecoin Volatility)
HARD_STOP_LOSS_PCT = -0.15    # -15% absoluter Stop-Loss (Notbremse)
MIN_PROFIT_TO_EXIT = 0.025    # Mind. 2.5% Profit bevor Trailing Stop greift

# ── Dynamic Trailing Stop (Step-Up) ─────────────────────────────────────────
STEP_UP_1_PROFIT = 0.20       # Wenn Trade > +20% Profit hat...
STEP_UP_1_TRAILING = 0.15     # ...erweitere Trailing Stop auf 15% (Coin darf mehr atmen)
STEP_UP_2_PROFIT = 0.50       # Wenn Trade > +50% Profit hat (Moon-Phase)...
STEP_UP_2_TRAILING = 0.25     # ...erweitere Trailing Stop auf 25% (für die massiven 200%+ Runs)

# ── Pump-Tracking (erkennt NEUE Pumps vs. alte) ────────────────────────────
PUMP_COOLDOWN_MIN = 30        # Gleichen Coin frühestens nach 30 Min nochmal kaufen

# ── Circuit Breaker (Global Market Health) ──────────────────────────────────
MARKET_HEALTH_CHECK_INTERVAL_MIN = 15  # Wie oft den Gesamtmarkt prüfen (Minuten)
MIN_GREEN_RATIO_PCT = 0.15             # Mindestens 15% aller Coins müssen positiv sein (vorher 25%)
MAX_BTC_ETH_DROP_PCT = -3.0           # Wenn BTC/ETH mehr als -3.0% in 24h abstürzen -> Toxic Market
GLOBAL_COOLDOWN_HOURS = 2             # Bot pausiert Kaufe für 2h wenn Markt "Toxic" ist

# ══════════════════════════════════════════════════════════════════════════════
#  LOGGING
# ══════════════════════════════════════════════════════════════════════════════

log_dir = Path(__file__).parent
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s │ %(levelname)-7s │ %(message)s",
    datefmt="%H:%M:%S",
    handlers=[
        logging.StreamHandler(sys.stdout),
        logging.FileHandler(log_dir / "autotrader.log", encoding="utf-8"),
    ]
)
log = logging.getLogger("autotrader")


# ══════════════════════════════════════════════════════════════════════════════
#  DATENTYPEN
# ══════════════════════════════════════════════════════════════════════════════

class TelegramNotifier:
    def __init__(self, token: str, chat_id: str, enabled: bool):
        self.token = token
        self.chat_id = str(chat_id)
        self.enabled = enabled

    def send(self, msg: str):
        if not self.enabled or not self.token or not self.chat_id:
            return
        
        url = f"https://api.telegram.org/bot{self.token}/sendMessage"
        data = urllib.parse.urlencode({"chat_id": self.chat_id, "text": msg, "parse_mode": "HTML"}).encode('utf-8')
        try:
            req = urllib.request.Request(url, data=data)
            with urllib.request.urlopen(req, timeout=5) as resp:
                pass
        except Exception as e:
            log.warning(f"  ⚠️  Telegram-Fehler: {e}")

@dataclass
class OpenTrade:
    pair: str
    entry_price: float
    amount: float
    stake_eur: float
    entry_time: datetime
    highest_price: float = 0.0
    order_id: str = ""
    stop_loss_order_id: str = ""
    trailing_stop_pct: float = 0.10
    hard_sl_pct: float = -0.15

    def profit_pct(self, current_price: float) -> float:
        return (current_price - self.entry_price) / self.entry_price

    def profit_eur(self, current_price: float) -> float:
        return self.stake_eur * self.profit_pct(current_price)

    def time_in_trade_min(self) -> float:
        return (datetime.now(timezone.utc) - self.entry_time).total_seconds() / 60

    def drawdown_from_high(self, current_price: float) -> float:
        """Wie weit ist der Preis vom Höchststand gefallen?"""
        if self.highest_price <= 0:
            return 0.0
        return (current_price - self.highest_price) / self.highest_price


# ══════════════════════════════════════════════════════════════════════════════
#  MOMENTUM AUTOTRADER
# ══════════════════════════════════════════════════════════════════════════════

class MomentumTrader:
    def __init__(self, api_key: str = None, api_secret: str = None, tele_token: str = None, tele_chat: str = None):
        self.open_trades: dict[str, OpenTrade] = {}  # pair -> OpenTrade
        self.trade_history: list[dict] = []
        self.total_profit = 0.0
        self.eur_balance = 0.0
        self.all_eur_pairs: list[str] = []
        self.pump_cooldowns: dict[str, float] = {}  # pair → timestamp
        self.prev_tickers: dict[str, float] = {}    # pair → last_price (für Trend)
        self.price_history: dict[str, list[tuple[float, float]]] = {}  # pair → [(timestamp, price)] (2h Historie)
        
        # Phase 3 Circuit Breaker States
        self.market_is_toxic: bool = False
        self.global_cooldown_until: float = 0.0
        self.last_market_health_check: float = 0.0
        self.last_telegram_summary: float = 0.0  # Phase 11: Hourly Summary Timer
        
        self.api_key = api_key
        self.api_secret = api_secret
        
        config = {}
        if CONFIG_FILE.exists():
            try:
                with open(CONFIG_FILE) as f:
                    config = json.load(f)
            except Exception:
                pass

        tele_conf = config.get("telegram", {})
        final_token = tele_token or tele_conf.get("token", "")
        final_chat = tele_chat or tele_conf.get("chat_id", "")
        
        self.notifier = TelegramNotifier(
            token=final_token,
            chat_id=final_chat,
            enabled=bool(final_token and final_chat) or tele_conf.get("enabled", False)
        )
        
        self.exchange = self._init_exchange(config)

    def _init_exchange(self, config: dict) -> ccxt.kraken:
        try:
            kr_key = self.api_key or config.get("exchange", {}).get("key", "")
            kr_sec = self.api_secret or config.get("exchange", {}).get("secret", "")

            exchange = ccxt.kraken({
                "apiKey": kr_key,
                "secret": kr_sec,
                "enableRateLimit": True,
                "rateLimit": 3000,
                "nonce": lambda: int(time.time() * 1000000),
                "options": {"adjustForTimeDifference": True},
            })

            exchange.load_markets()

            self.all_eur_pairs = [
                symbol for symbol, market in exchange.markets.items()
                if (market.get("quote") == STAKE_CURRENCY
                    and market.get("active", True)
                    and market.get("spot", True)
                    and "/" in symbol
                    and not symbol.endswith(".d"))
            ]

            self.exchange = exchange  # Setzen vor _refresh_balance

            log.info(f"✅ Kraken verbunden | {len(self.all_eur_pairs)} EUR-Pairs")

            if DRY_RUN:
                log.info("🧪 DRY RUN MODUS")
                self.eur_balance = 1000.0
            else:
                log.warning("⚠️  LIVE MODUS – Echte Trades!")
                self._consolidate_holdings()
                self._detect_existing_position()
                self._refresh_balance()
                log.info(f"  💰 EUR verfügbar: €{self.eur_balance:.2f}")

            return exchange

        except Exception as e:
            log.error(f"❌ Verbindung fehlgeschlagen: {e}")
            sys.exit(1)

    def _refresh_balance(self):
        try:
            balance = self.exchange.fetch_balance()
            self.eur_balance = float(balance.get("EUR", {}).get("free", 0))
        except Exception as e:
            log.warning(f"  ⚠️  Balance-Fehler: {e}")

    def _consolidate_holdings(self):
        """Verkaufe alle kleinen Holdings zu EUR – maximiere Kapital."""
        try:
            balance = self.exchange.fetch_balance()
            skip = {"EUR", "CHF", "USD", "GBP", "CAD", "AUD", "JPY"}

            # Sammle alle Holdings mit ihrem EUR-Wert
            holdings = []
            for coin, vals in balance.get("total", {}).items():
                if coin in skip:
                    continue
                amount = float(vals) if vals else 0
                if amount <= 0:
                    continue

                pair = f"{coin}/EUR"
                if pair not in self.exchange.markets:
                    continue

                try:
                    ticker = self.exchange.fetch_ticker(pair)
                    price = ticker.get("last", 0) or 0
                    if price <= 0:
                        continue
                    value = amount * price
                    holdings.append((coin, pair, amount, price, value))
                    time.sleep(1)
                except Exception:
                    continue

            if not holdings:
                return

            # Sortiere nach Wert – größte Position ist der "Trade"
            holdings.sort(key=lambda x: x[4], reverse=True)
            biggest = holdings[0]

            log.info(f"  📦 Holdings gefunden:")
            for coin, pair, amount, price, value in holdings:
                is_main = "👑" if coin == biggest[0] else "💱"
                log.info(f"     {is_main} {pair}: {amount:.6f} = €{value:.2f}")

            # Alle AUSSER der größten Position verkaufen
            for coin, pair, amount, price, value in holdings[1:]:
                if value < 0.50:  # Unter 50 Cent nicht verkaufen (Dust)
                    continue

                # Mindest-Order prüfen
                market = self.exchange.markets.get(pair, {})
                min_amount = market.get("limits", {}).get("amount", {}).get("min", 0) or 0
                if min_amount > 0 and amount < min_amount:
                    log.info(f"     ⏭️  {pair}: Unter Mindestmenge ({amount} < {min_amount})")
                    continue

                try:
                    # Sicherstellen dass keine offenen Orders den Verkauf blockieren
                    try:
                        self.exchange.cancel_all_orders(pair)
                    except Exception:
                        pass

                    # Frische Balance holen für diesen Coin
                    bal = self.exchange.fetch_balance()
                    free_amt = float(bal.get("free", {}).get(coin, 0))
                    
                    if free_amt <= 0:
                        continue

                    # 0.01% Puffer und Präzision
                    amt_to_sell = float(self.exchange.amount_to_precision(pair, free_amt * 0.9999))
                    
                    if amt_to_sell <= 0:
                        continue

                    order = self.exchange.create_market_sell_order(pair, amt_to_sell)
                    sell_price = order.get("average") or order.get("price") or price
                    log.info(f"     🔄 VERKAUFT {pair}: {amt_to_sell:.6f} → ~€{value:.2f}")
                    time.sleep(2)
                except Exception as e:
                    log.warning(f"     ⚠️  Konnte {pair} nicht verkaufen (Free: {free_amt if 'free_amt' in locals() else '?'}): {e}")

        except Exception as e:
            log.warning(f"  ⚠️  Holdings-Check fehlgeschlagen: {e}")

    def _detect_existing_position(self):
        """Prüfe ob wir schon einen Coin halten (z.B. nach Neustart)."""
        try:
            balance = self.exchange.fetch_balance()
            skip = {"EUR", "CHF", "USD", "GBP", "CAD", "AUD", "JPY"}

            for coin, vals in balance.get("total", {}).items():
                if coin in skip:
                    continue
                amount = float(vals) if vals else 0
                if amount <= 0:
                    continue

                # Prüfe ob es genug Wert hat
                pair = f"{coin}/EUR"
                if pair not in self.exchange.markets:
                    continue

                try:
                    ticker = self.exchange.fetch_ticker(pair)
                    price = float(ticker.get("last", 0) or 0)
                    if price <= 0:
                        continue
                    value = amount * price
                    if value < 1.0:
                        continue

                    # Echten Einstiegspreis aus Historie suchen
                    entry_price = price
                    try:
                        # Hole letzte Trades für dieses Pair
                        trades = self.exchange.fetch_my_trades(pair, limit=5)
                        if trades:
                            # Nimm den Preis des letzten Kaufs
                            buy_trades = [t for t in trades if t["side"] == "buy"]
                            if buy_trades:
                                entry_price = float(buy_trades[-1]["price"])
                                log.info(f"     ✅ Echter Einstiegspreis gefunden: {entry_price:.6f}")
                    except Exception as e:
                        log.warning(f"     ⚠️  Konnte echten Preis nicht laden, nutze aktuellen: {e}")

                    # Bestehende Position übernehmen!
                    self.open_trades[pair] = OpenTrade(
                        pair=pair, entry_price=entry_price, amount=amount,
                        stake_eur=amount * entry_price, entry_time=datetime.now(timezone.utc),
                        highest_price=max(price, entry_price), order_id="EXISTING",
                        trailing_stop_pct=TRAILING_STOP_PCT, hard_sl_pct=HARD_STOP_LOSS_PCT
                    )
                    log.info(f"  📦 Bestehende Position erkannt: {pair} | "
                             f"{amount:.6f} coins | ~€{value:.2f}")
                    if len(self.open_trades) >= MAX_OPEN_TRADES:
                        return

                except Exception:
                    continue

        except Exception as e:
            log.warning(f"  ⚠️  Position-Check fehlgeschlagen: {e}")

    # ──────────────────────────────────────────────────────────────────────
    #  CIRCUIT BREAKER: Global Market Health
    # ──────────────────────────────────────────────────────────────────────

    def check_global_market_health(self) -> tuple[bool, str]:
        """Prüft BTC/ETH und das Gesamtverhältnis von grünen zu roten Coins. Gibt (is_toxic, reason) aus."""
        now = time.time()
        
        # Nur alle X Minuten checken um Rate Limits zu schonen
        if now - self.last_market_health_check < (MARKET_HEALTH_CHECK_INTERVAL_MIN * 60):
            return self.market_is_toxic, "Cached"
            
        self.last_market_health_check = now

        try:
            # Hole Ticker für die Leitwährungen
            leaders = self.exchange.fetch_tickers(['BTC/EUR', 'ETH/EUR'])
            btc_pct = leaders.get('BTC/EUR', {}).get('percentage', 0) or 0
            eth_pct = leaders.get('ETH/EUR', {}).get('percentage', 0) or 0
            
            # Kritischer Check 1: Bluten die Majors?
            if btc_pct < MAX_BTC_ETH_DROP_PCT and eth_pct < MAX_BTC_ETH_DROP_PCT:
                self.market_is_toxic = True
                reason = f"Majors dumping (BTC: {btc_pct:.1f}%, ETH: {eth_pct:.1f}%)"
                return True, reason

            # Hole alle Ticker für Ratio-Check
            # Das ist etwas teurer, machen wir deshalb nur alle 15 Min
            all_tickers = self.exchange.fetch_tickers(self.all_eur_pairs)
            if not all_tickers:
                return self.market_is_toxic, "API Error"
                
            green_count = 0
            total_valid = 0
            
            for symbol, ticker in all_tickers.items():
                if symbol not in self.all_eur_pairs:
                    continue
                pct = ticker.get('percentage', 0) or 0
                if pct > 0:
                    green_count += 1
                total_valid += 1
                
            if total_valid == 0:
                return self.market_is_toxic, "No total valid"
                
            green_ratio = green_count / total_valid
            
            # Kritischer Check 2: Ist der breite Markt rot?
            if green_ratio < MIN_GREEN_RATIO_PCT:
                self.market_is_toxic = True
                reason = f"Market bleeding (Only {green_ratio*100:.1f}% pairs are green)"
                return True, reason

            # Markt ist okay!
            self.market_is_toxic = False
            return False, f"Healthy (BTC: {btc_pct:.1f}%, Green Ratio: {green_ratio*100:.1f}%)"

        except Exception as e:
            log.warning(f"  ⚠️  Market Health Check failed: {e}")
            return self.market_is_toxic, f"Error: {e}"

    # ──────────────────────────────────────────────────────────────────────
    #  PUMP-ERKENNUNG (1 API-Call für ALLE Pairs!)
    # ──────────────────────────────────────────────────────────────────────

    def detect_pumps(self) -> list[tuple[str, float, float, float, float]]:
        """
        Scannt alle EUR-Pairs via Bulk-Ticker.
        Gibt Liste von (pair, pct_change, volume_eur, price, volatility) zurück,
        sortiert nach stärkstem Pump.
        """
        try:
            tickers = self.exchange.fetch_tickers(self.all_eur_pairs)
        except Exception as e:
            log.error(f"  ❌ Ticker-Fehler: {e}")
            return []

        pumps = []
        now = time.time()

        for symbol, ticker in tickers.items():
            if symbol not in self.all_eur_pairs:
                continue

            price = ticker.get("last", 0) or 0
            pct_24h = ticker.get("percentage", 0) or 0
            base_vol = ticker.get("baseVolume", 0) or 0
            quote_vol = ticker.get("quoteVolume", 0) or 0

            # Volumen in EUR
            vol_eur = quote_vol if quote_vol > 0 else (base_vol * price)

            # --- Phase 10: Rolling Price History (2 Hours) ---
            if symbol not in self.price_history:
                self.price_history[symbol] = []
            self.price_history[symbol].append((now, price))
            
            # Speicher bereinigen: Nur die letzten 7200 Sekunden (2 Stunden) behalten
            self.price_history[symbol] = [(t, p) for (t, p) in self.price_history[symbol] if now - t <= 7200]

            # Filter anwenden
            if price < PUMP_MIN_PRICE:
                continue
            if vol_eur < PUMP_MIN_VOLUME_EUR:
                continue
            if pct_24h < PUMP_MIN_PCT_24H:
                continue
            if pct_24h > PUMP_MAX_ALREADY_PUMPED:
                continue

            # Cooldown prüfen
            cooldown_until = self.pump_cooldowns.get(symbol, 0)
            if now < cooldown_until:
                continue

            # Preis-Trend prüfen: steigt der Coin GERADE noch?
            prev_price = self.prev_tickers.get(symbol, 0)
            if prev_price > 0 and price < prev_price:
                continue  # Preis fällt gerade → kein Entry

            # --- Phase 10: Short-Term Momentum Filter ---
            history = self.price_history[symbol]
            price_15m_ago = None
            price_1h_ago = None
            
            # Suche die passenden historischen Preise (history ist chronologisch sortiert)
            for t, p in history:
                if now - t <= 900 and price_15m_ago is None:
                    price_15m_ago = p    # Ältester Preis innerhalb der letzten 15 Min
                if now - t <= 3600 and price_1h_ago is None:
                    price_1h_ago = p     # Ältester Preis innerhalb der letzten 1 Std
                    
            if not price_15m_ago: price_15m_ago = price
            if not price_1h_ago: price_1h_ago = price
            
            pct_15m = (price - price_15m_ago) / price_15m_ago * 100 if price_15m_ago > 0 else 0.0
            pct_1h = (price - price_1h_ago) / price_1h_ago * 100 if price_1h_ago > 0 else 0.0
            
            # Kriterium 1: Coin darf nicht gerade stark fallen (-0.5% in 15m)
            if pct_15m < -0.5:
                continue
                
            # Kriterium 2: Der Pump muss aktuell sein (nicht von gestern Nacht).
            # Wir fordern entweder frische +1% in 15m ODER beständige +2% in 1h.
            # (Wenn der Bot neu startet, ist der Speicher leer -> er kauft erst, wenn er den Pump "live" sieht!)
            if pct_15m < PUMP_MIN_PCT_15M and pct_1h < PUMP_MIN_PCT_1H:
                continue

            # Volatility ermitteln (high / low var)
            high_24h = ticker.get("high", 0) or 0
            low_24h = ticker.get("low", 0) or 0
            volatility = 0.0
            if low_24h > 0:
                volatility = (high_24h - low_24h) / low_24h

            pumps.append((symbol, pct_24h, vol_eur, price, volatility))

        # Vorherige Preise speichern für nächsten Zyklus
        for symbol, ticker in tickers.items():
            p = ticker.get("last", 0) or 0
            if p > 0:
                self.prev_tickers[symbol] = p

        # Sortiere: stärkster Pump zuerst, aber bevorzuge frische Pumps
        # Score = pct_change * log(volume) → hoher Pump + hohes Volumen
        import math
        pumps.sort(key=lambda x: x[1] * math.log10(max(x[2], 1)), reverse=True)

        return pumps

    # ──────────────────────────────────────────────────────────────────────
    #  TRAILING STOP EXIT
    # ──────────────────────────────────────────────────────────────────────

    def check_exit(self, pair: str, current_price: float) -> str | None:
        """Prüfe ob der offene Trade geschlossen werden soll."""
        trade = self.open_trades.get(pair)
        if not trade:
            return None

        profit = trade.profit_pct(current_price)

        # Höchststand tracken
        if current_price > trade.highest_price:
            trade.highest_price = current_price

        # 1. Hard Stop-Loss (Notbremse)
        if profit <= trade.hard_sl_pct:
            return f"🛑 STOP-LOSS ({profit*100:.1f}% / SL: {trade.hard_sl_pct*100:.1f}%)"

        # 2. Dynamic Trailing Stop (Step-Up)
        if profit > MIN_PROFIT_TO_EXIT:
            # Berechne den dynamischen Stop basierend auf dem aktuellen Höchststand (Profit am Peak)
            peak_profit = trade.profit_pct(trade.highest_price)
            
            # Wähle den passenden Trailing Stop für diese Profit-Stufe
            active_trailing_stop = trade.trailing_stop_pct  # Standard (z.B. 10%)
            
            if peak_profit >= STEP_UP_2_PROFIT:
                active_trailing_stop = max(trade.trailing_stop_pct, STEP_UP_2_TRAILING) # z.B. 25%
            elif peak_profit >= STEP_UP_1_PROFIT:
                active_trailing_stop = max(trade.trailing_stop_pct, STEP_UP_1_TRAILING) # z.B. 15%

            drawdown = trade.drawdown_from_high(current_price)
            if drawdown <= -active_trailing_stop:
                stufe = ""
                if active_trailing_stop == STEP_UP_2_TRAILING: stufe = " [MOON-PHASE 25%]"
                elif active_trailing_stop == STEP_UP_1_TRAILING: stufe = " [STEP-UP 15%]"
                
                return (f"📉 TRAILING STOP{stufe} | Peak: {trade.highest_price:.6f} → "
                        f"Now: {current_price:.6f} ({drawdown*100:.1f}% vom High)")

        return None

    # ──────────────────────────────────────────────────────────────────────
    #  ORDER EXECUTION
    # ──────────────────────────────────────────────────────────────────────

    def execute_buy(self, pair: str, price: float, volatility: float = 0.0) -> bool:
        """Kauforder ausfuehren. Gibt True bei Erfolg zurueck."""
        if pair in self.open_trades:
            return False

        if len(self.open_trades) >= MAX_OPEN_TRADES:
            return False

        if not DRY_RUN:
            self._refresh_balance()

        # TEIL-EINSATZ BERECHNEN (Slot System)
        remaining_slots = MAX_OPEN_TRADES - len(self.open_trades)
        if remaining_slots <= 0:
            return False
            
        # Nimm den verfügbaren Betrag, geteilt durch die restlichen Slots, aber maximal alles, minus 1% Gebührenpuffer
        slot_stake = (self.eur_balance / remaining_slots) * 0.99
        stake = min(slot_stake, self.eur_balance * 0.99)

        if stake < 1.0:
            log.warning(f"  ⚠️  Zu wenig EUR für neuen Slot (€{self.eur_balance:.2f} total / €{stake:.2f} berechnet)")
            return False

        # Mindest-Ordergroesse pruefen
        market = self.exchange.markets.get(pair, {})
        min_amount = market.get("limits", {}).get("amount", {}).get("min", 0) or 0
        min_cost = market.get("limits", {}).get("cost", {}).get("min", 0) or 0
        amount = stake / price

        if min_cost > 0 and stake < min_cost:
            log.info(f"  ⏭️  {pair}: Mindest-Order €{min_cost:.2f} > Stake €{stake:.2f}")
            return False
        if min_amount > 0 and amount < min_amount:
            log.info(f"  ⏭️  {pair}: Mindest-Menge {min_amount} > {amount:.8f}")
            return False

        amount = stake / price
        
        # --- PHASE 2: DYNAMIC "REGIME" STRATEGY ---
        if volatility > 0.40:
            t_stop = 0.15 # 15% trailing stop for hyper-volatile memecoins
            h_stop = -0.20 # -20% hard stop
            regime = "Hyper-Volatile"
        elif volatility > 0.15:
            t_stop = 0.10 # 10% trailing stop for normal memecoins
            h_stop = -0.15
            regime = "Volatile Meme"
        else:
            t_stop = 0.06 # 6% trailing stop for calm alts (e.g. BTC/ETH)
            h_stop = -0.10
            regime = "Calm Altcoin"
            
        # Optional: override falls Config global groesser angesetzt ist als berechnet
        t_stop = max(t_stop, TRAILING_STOP_PCT)
        h_stop = min(h_stop, HARD_STOP_LOSS_PCT)

        if DRY_RUN:
            self.open_trades[pair] = OpenTrade(
                pair=pair, entry_price=price, amount=amount,
                stake_eur=stake, entry_time=datetime.now(timezone.utc),
                highest_price=price, order_id=f"DRY-{int(time.time())}",
                stop_loss_order_id=f"DRY-SL-{int(time.time())}",
                trailing_stop_pct=t_stop, hard_sl_pct=h_stop
            )
            self.eur_balance -= stake
            log.info(f"  🟢 BUY  {pair} | €{stake:.2f} (SLOT {len(self.open_trades)}/{MAX_OPEN_TRADES}) @ {price:.6f} [DRY]")
            log.info(f"  🧠 Regime: {regime} | Volatility: {volatility*100:.1f}% -> Trailing: {t_stop*100:.1f}%, SL: {h_stop*100:.1f}%")
        else:
            try:
                order = self.exchange.create_market_buy_order(pair, amount)
                fill_price = float(order.get("average") or order.get("price") or price)
                fill_amount = float(order.get("filled") or order.get("amount") or amount)
                
                # Berechne den Exchange-Side Stop-Loss Preis
                sl_price = fill_price * (1.0 + h_stop)
                
                sl_order_id = ""
                try:
                    # Kraken Stop-Loss Order platzieren: Typ ist "stop-loss", price ist der Auslösepreis
                    sl_order = self.exchange.create_order(
                        symbol=pair, 
                        type="stop-loss", 
                        side="sell", 
                        amount=fill_amount, 
                        price=sl_price, 
                        # Für manche Exchanges braucht es stopLossPrice param, bei Kraken oft direkt als price argument oder beides zur Sicherheit
                        params={"stopLossPrice": sl_price} 
                    )
                    sl_order_id = str(sl_order.get("id", ""))
                    log.info(f"  🛡️  Hard Stop-Loss order placed on Kraken at {sl_price:.6f} (ID: {sl_order_id})")
                except Exception as sl_err:
                    log.error(f"  ⚠️  Konnte Stop-Loss bei Kraken nicht platzieren: {sl_err}")

                self.open_trades[pair] = OpenTrade(
                    pair=pair, entry_price=fill_price, amount=fill_amount,
                    stake_eur=stake, entry_time=datetime.now(timezone.utc),
                    highest_price=fill_price, order_id=str(order.get("id", "")),
                    stop_loss_order_id=sl_order_id,
                    trailing_stop_pct=t_stop, hard_sl_pct=h_stop
                )
                
                # Wir vertrauen nach einem Trade der Exchange-Balance anstatt es selbst zu rechnen, daher kein -= stake
                log.info(f"  🟢 BUY  {pair} | €{stake:.2f} (SLOT {len(self.open_trades)}/{MAX_OPEN_TRADES}) @ {fill_price:.6f} | Order: {order.get('id', 'N/A')}")
                log.info(f"  🧠 Regime: {regime} | Volatility: {volatility*100:.1f}% -> Trailing: {t_stop*100:.1f}%, SL: {h_stop*100:.1f}%")
                self.notifier.send(f"🟢 <b>BUY</b> {pair}\nStake: €{stake:.2f} (SLOT {len(self.open_trades)}/{MAX_OPEN_TRADES})\nPrice: <code>{fill_price:.6f}</code>\n🧠 <b>Regime:</b> {regime} (V: {volatility*100:.1f}%)\nTS: {t_stop*100:.1f}%, SL: {h_stop*100:.1f}%")
            except Exception as e:
                log.error(f"  ❌ BUY FAILED {pair}: {e}")
                self.notifier.send(f"❌ <b>BUY FAILED</b> {pair}\nError: <code>{e}</code>")
                # Cooldown setzen, damit wir diesen Coin nicht sofort wieder versuchen
                self.pump_cooldowns[pair] = time.time() + (PUMP_COOLDOWN_MIN * 60)
                return False
        return True

    def execute_sell(self, pair: str, reason: str, current_price: float):
        trade = self.open_trades.get(pair)
        if not trade:
            return

        profit_pct = trade.profit_pct(current_price)
        profit_eur = trade.profit_eur(current_price)

        if DRY_RUN:
            log.info(f"  🔴 SELL {trade.pair} | {profit_pct*100:+.2f}% (€{profit_eur:+.2f}) | {reason} [DRY]")
            self.eur_balance = trade.stake_eur + profit_eur
            self.total_profit += profit_eur
            
            # Telegram Notifier (Clean & Concise)
            self.notifier.send(f"🔴 <b>DRY SELL: {pair}</b>\n"
                               f"<b>P/L:</b> {profit_pct*100:+.2f}% (€{profit_eur:+.2f})\n"
                               f"<b>Reason:</b> {reason}\n"
                               f"<b>Duration:</b> {trade.time_in_trade_min():.0f} min\n"
                               f"<b>Balance:</b> €{self.eur_balance:.2f}")
        else:
            try:
                # KRITISCHER BEFEHL: Wenn es eine offenen Kraken Stop-Loss gibt, müssen wir den abbrechen, 
                # BEVOR wir market-sellen, sonst verkaufen wir Coins die evtl blockiert sind.
                if trade.stop_loss_order_id:
                    try:
                        self.exchange.cancel_order(id=trade.stop_loss_order_id, symbol=trade.pair)
                        log.info(f"  🛡️  Hard Stop-Loss canceled (ID: {trade.stop_loss_order_id})")
                    except Exception as cancel_err:
                        log.warning(f"  ⚠️  Fehler beim Canceln der SL Order (wurde evtl. schon von Kraken ausgelöst): {cancel_err}")

                # KRITISCHER FIX: Hole das tatsächliche Guthaben des Coins
                # Nutze einen winzigen Puffer (0.01%) und die korrekte Kraken-Präzision um "Insufficient funds" zu vermeiden
                base_coin = trade.pair.split('/')[0]
                self._refresh_balance() # Sicherstellen dass wir frische Daten haben
                balance = self.exchange.fetch_balance()
                free_balance = float(balance.get("free", {}).get(base_coin, 0))
                
                # Wir nehmen 99.99% des Guthabens (0.01% Puffer) und runden auf Kraken-Format
                raw_amount = free_balance * 0.9999
                actual_amount_to_sell = float(self.exchange.amount_to_precision(trade.pair, raw_amount))

                if actual_amount_to_sell <= 0:
                     log.error(f"  ❌ SELL FAILED {trade.pair}: Kein ausreichendes Guthaben von {base_coin} gefunden (Free: {free_balance})!")
                     self.notifier.send(f"❌ <b>SELL FAILED</b> {trade.pair}\nKein Guthaben von {base_coin} gefunden!")
                     self.open_trades.pop(pair, None) # Positions-Deadlock auflösen
                     return

                log.info(f"  🔄 Vorbereiten SELL für {trade.pair}: Versuche {actual_amount_to_sell:.6f} {base_coin} zu verkaufen (Verfügbar: {free_balance:.6f})")

                order = self.exchange.create_market_sell_order(trade.pair, actual_amount_to_sell)
                actual_price = order.get("average") or order.get("price") or current_price
                
                # Konvertiere zu float für Berechnungen
                actual_price = float(actual_price)
                
                profit_pct = trade.profit_pct(actual_price)
                profit_eur = trade.profit_eur(actual_price)
                log.info(f"  🔴 SELL {trade.pair} | {profit_pct*100:+.2f}% (€{profit_eur:+.2f}) | {reason}")
                self._refresh_balance()
                self.total_profit += profit_eur
                
                # Telegram Notifier (Clean & Concise)
                self.notifier.send(f"🔴 <b>SELL: {pair}</b>\n"
                                   f"<b>P/L:</b> {profit_pct*100:+.2f}% (€{profit_eur:+.2f})\n"
                                   f"<b>Reason:</b> {reason}\n"
                                   f"<b>Duration:</b> {trade.time_in_trade_min():.0f} min\n"
                                   f"<b>Balance:</b> €{self.eur_balance:.2f}")

            except Exception as e:
                log.error(f"  ❌ SELL FAILED {trade.pair}: {e}")
                self.notifier.send(f"❌ <b>SELL FAILED</b> {trade.pair}\nError: <code>{e}</code>")
                self.open_trades.pop(pair, None) # Wenn Sell absolut fehlschlägt, Bug verhindern, dass er festhängt
                return


        self.trade_history.append({
            "pair": trade.pair, "entry": trade.entry_price,
            "exit": current_price, "profit_pct": profit_pct,
            "profit_eur": profit_eur, "reason": reason,
            "duration_min": trade.time_in_trade_min(),
        })

        # Cooldown setzen
        self.pump_cooldowns[trade.pair] = time.time() + (PUMP_COOLDOWN_MIN * 60)
        self.open_trades.pop(pair, None)

    # ──────────────────────────────────────────────────────────────────────
    #  LIVE PRICE CHECK (für offenen Trade)
    # ──────────────────────────────────────────────────────────────────────

    def get_current_price(self, pair: str) -> float | None:
        try:
            ticker = self.exchange.fetch_ticker(pair)
            return ticker.get("last", 0)
        except Exception as e:
            log.error(f"  ❌ Preis-Fehler {pair}: {e}")
            return None

    # ──────────────────────────────────────────────────────────────────────
    #  MAIN LOOP
    # ──────────────────────────────────────────────────────────────────────

    def run(self):
        mode = "🧪 DRY" if DRY_RUN else "⚠️  LIVE"
        log.info("═" * 60)
        log.info(f"  🚀 Momentum Trader gestartet [{mode}]")
        log.info(f"  📊 {len(self.all_eur_pairs)} EUR-Pairs | Alle {CHECK_INTERVAL}s")
        log.info(f"  💰 Balance: €{self.eur_balance:.2f} ({MAX_OPEN_TRADES} Slots verfügbar)")
        log.info(f"  📈 Pump-Erkennung: >{PUMP_MIN_PCT_24H}% (24h) & Frisches Momentum (15m/1h) | Vol >€{PUMP_MIN_VOLUME_EUR}")
        log.info(f"  📉 Trailing Stop: {TRAILING_STOP_PCT*100}% | Hard SL: {HARD_STOP_LOSS_PCT*100}%")
        log.info("═" * 60)
        
        self.notifier.send(f"🤖 <b>Momentum Trader gestartet</b>\nModus: {mode}\nPairs: {len(self.all_eur_pairs)}\nBalance: €{self.eur_balance:.2f}\nSlots: {MAX_OPEN_TRADES}")

        cycle = 0
        while True:
            try:
                cycle += 1

                # ── OFFENE TRADES: Prüfe Exit ────────────────────────
                pairs_to_sell = []
                for pair, trade in list(self.open_trades.items()):
                    price = self.get_current_price(pair)
                    if price:
                        profit = trade.profit_pct(price)
                        highest = trade.highest_price
                        dd = trade.drawdown_from_high(price)

                        if price > highest:
                            trade.highest_price = price
                            log.info(f"  🔥 {pair}: NEUES HIGH! {price:.6f} "
                                     f"({profit*100:+.2f}%)")
                        else:
                            log.info(f"  📈 SLOT {list(self.open_trades.keys()).index(pair)+1}/{MAX_OPEN_TRADES} - {pair}: {profit*100:+.2f}% | "
                                     f"High: {highest:.6f} | DD: {dd*100:.1f}% | "
                                     f"{trade.time_in_trade_min():.0f}min")

                        exit_reason = self.check_exit(pair, price)
                        if exit_reason:
                            pairs_to_sell.append((pair, exit_reason, price))

                for pair, reason, price in pairs_to_sell:
                    self.execute_sell(pair, reason, price)

                # ── NEUE PUMPS SUCHEN ──────────────────────────
                if cycle % 5 == 1 or cycle == 1:
                    slots_used = len(self.open_trades)
                    log.info(f"──── Zyklus {cycle} | {datetime.now().strftime('%H:%M:%S')} | "
                             f"€{self.eur_balance:.2f} | Slots: {slots_used}/{MAX_OPEN_TRADES} | P/L: €{self.total_profit:+.2f} ────")
                             
                now = time.time()
                
                # Check Global Cooldown
                if self.global_cooldown_until > now:
                    if cycle % 120 == 0:  # Ca. alle 10 Minuten
                        remaining = (self.global_cooldown_until - now) / 3600
                        log.info(f"  🛡️  CIRCUIT BREAKER AKTIV: Pausiere Käufe für weitere {remaining:.1f}h")
                    continue  # Überspringe Pump-Erkennung komplett

                # Check Market Health
                if len(self.open_trades) < MAX_OPEN_TRADES:
                    is_toxic, reason = self.check_global_market_health()
                    
                    if is_toxic and reason != "Cached":
                        log.warning(f"  🚨 TOXIC MARKET DETECTED: {reason}")
                        log.warning(f"  🛡️  Aktiviere Circuit Breaker für {GLOBAL_COOLDOWN_HOURS} Stunden.")
                        self.notifier.send(f"🛡️ <b>CIRCUIT BREAKER AKTIVIERT</b>\n"
                                          f"<b>Reason:</b> {reason}\n"
                                          f"Der Bot pausiert Käufe für <b>{GLOBAL_COOLDOWN_HOURS}h</b> um Kapital in diesem Umfeld zu schützen.\n"
                                          f"Offene Trades werden weiterhin regulär über Stop-Loss gemanaged.")
                        self.global_cooldown_until = now + (GLOBAL_COOLDOWN_HOURS * 3600)
                        
                        # Set interval to shorter for next test after sleep
                        self.last_market_health_check = 0 
                        continue

                pumps = self.detect_pumps()

                if pumps:
                    # Top 3 anzeigen (nur alle 5 Zyklen)
                    if cycle % 5 == 1 or cycle == 1:
                        for p, pc, v, pr, vola in pumps[:3]:
                            log.info(f"     {'🚀' if p == pumps[0][0] else '  '} {p}: +{pc:.1f}% (€{v:,.0f} | V: {vola*100:.1f}%)")

                    # 1. Normale Käufe (Wenn noch freie Slots da sind)
                    if len(self.open_trades) < MAX_OPEN_TRADES:
                        for pair, pct, vol, price, vola in pumps[:5]:
                            if pair in self.open_trades: continue
                            log.info(f"  🚀 PUMP: {pair} +{pct:.1f}% | Vol: €{vol:,.0f} | Kaufe!")
                            success = self.execute_buy(pair, price, vola)
                            if success and len(self.open_trades) >= MAX_OPEN_TRADES:
                                break  # Alle Slots voll
                                
                    # 2. Opportunity Cost Reallocation (Wenn Slots voll sind)
                    elif len(self.open_trades) >= MAX_OPEN_TRADES:
                        worst_stagnant_pair = None
                        worst_stagnant_profit = 999.0
                        
                        # Suche den "schlechtesten" stagnierenden Trade
                        for pair, trade in self.open_trades.items():
                            mins_running = trade.time_in_trade_min()
                            current_price = self.get_current_price(pair)
                            if not current_price: continue
                            
                            profit = trade.profit_pct(current_price) * 100
                            
                            if mins_running >= STAGNATION_MINUTES and profit < STAGNATION_MAX_PROFIT_PCT:
                                if profit < worst_stagnant_profit:
                                    worst_stagnant_profit = profit
                                    worst_stagnant_pair = pair
                                    
                        # Wenn wir einen Todeskandidaten gefunden haben
                        if worst_stagnant_pair:
                            best_new_pump = None
                            for pair, pct, vol, price, vola in pumps:
                                if pair not in self.open_trades:
                                    # pumps ist nach stärkstem Momentum sortiert
                                    best_new_pump = (pair, pct, price, vola)
                                    break
                                    
                            if best_new_pump:
                                new_pair, new_pct, new_price, new_vola = best_new_pump
                                mins_running = self.open_trades[worst_stagnant_pair].time_in_trade_min()
                                
                                log.warning(f"  🔄 REALLOCATION: Opfere stagnierenden {worst_stagnant_pair} "
                                            f"({worst_stagnant_profit:+.1f}%, {mins_running:.0f}min) "
                                            f"für starken neuen Pump {new_pair} ({new_pct:+.1f}%)")
                                            
                                self.notifier.send(f"🔄 <b>REALLOCATION</b>\n"
                                                  f"Opfere <b>{worst_stagnant_pair}</b> ({worst_stagnant_profit:+.1f}% nach {mins_running:.0f}m)\n"
                                                  f"für frischen Pump <b>{new_pair}</b> ({new_pct:+.1f}%)")
                                
                                # Verkaufe den lahmen Trade
                                current_price_stagnant = self.get_current_price(worst_stagnant_pair)
                                self.execute_sell(worst_stagnant_pair, f"Reallocated to {new_pair}", current_price_stagnant)
                                
                                time.sleep(1.5) # Kraken Guthaben Synch
                                
                                # Kaufe den frischen Pump
                                log.info(f"  🚀 PUMP: {new_pair} +{new_pct:.1f}% | REALLOCATION KAUF!")
                                self.execute_buy(new_pair, new_price, new_vola)
                                
                        # 3. Radar Fallback (Kein Telegram Spam mehr)
                        elif pumps[0][1] >= 15.0:  
                            pair, pct, vol, price, vola = pumps[0]
                            if time.time() > self.pump_cooldowns.get(pair + "_radar", 0):
                                log.info(f"  🚨 PUMP RADAR: {pair} +{pct:.1f}% (Slots fully in use!)")
                                # Radar Sends no telegram directly anymore to prevent spam. Handled in hourly summary.
                                self.pump_cooldowns[pair + "_radar"] = time.time() + (15 * 60)

                # Phase 11: Hourly Telegram Summary
                if now - self.last_telegram_summary > 3600:
                    self.last_telegram_summary = now
                    if len(self.trade_history) >= 0: # Only send if alive
                        wins = sum(1 for t in self.trade_history if t["profit_eur"] > 0)
                        losses = len(self.trade_history) - wins
                        
                        open_pos_text = ""
                        for pair, trade in self.open_trades.items():
                            curr_p = self.get_current_price(pair)
                            if curr_p:
                                prof = trade.profit_pct(curr_p) * 100
                                open_pos_text += f"• {pair}: {prof:+.1f}% ({trade.time_in_trade_min():.0f}m)\n"
                        if not open_pos_text:
                            open_pos_text = "Keine aktiven Trades."
                            
                        self.notifier.send(f"📊 <b>HOURLY SUMMARY</b>\n"
                                           f"<b>P/L Session:</b> €{self.total_profit:+.2f}\n"
                                           f"<b>Win/Loss:</b> {wins}W / {losses}L\n"
                                           f"<b>Balance:</b> €{self.eur_balance:.2f}\n"
                                           f"<b>Open Slots:</b> {MAX_OPEN_TRADES - len(self.open_trades)}/{MAX_OPEN_TRADES}\n\n"
                                           f"<b>Active Trades:</b>\n{open_pos_text}")

                # Status Console
                if self.trade_history and cycle % 10 == 0:
                    wins = sum(1 for t in self.trade_history if t["profit_eur"] > 0)
                    log.info(f"  📊 {wins}W/{len(self.trade_history)-wins}L | €{self.total_profit:+.2f}")

                time.sleep(CHECK_INTERVAL)

            except KeyboardInterrupt:
                log.info("\n\U0001f6d1 Bot gestoppt")
                if self.open_trades:
                    for pair, trade in self.open_trades.items():
                        p = self.get_current_price(pair)
                        if p:
                            profit = trade.profit_pct(p)
                            log.warning(f"  \u26a0\ufe0f  Offener Trade: {pair} "
                                        f"@ {trade.entry_price:.6f} ({profit*100:+.2f}%)")
                        else:
                            log.warning(f"  \u26a0\ufe0f  Offener Trade: {pair} "
                                        f"@ {trade.entry_price:.6f}")
                self._print_summary()
                break
            except Exception as e:
                log.error(f"❌ Fehler: {e}")
                time.sleep(10)

    def _print_summary(self):
        log.info("═" * 60)
        log.info(f"  📊 Total P/L: €{self.total_profit:+.2f} | Trades: {len(self.trade_history)}")
        if self.trade_history:
            wins = sum(1 for t in self.trade_history if t["profit_eur"] > 0)
            log.info(f"  W/L: {wins}/{len(self.trade_history)-wins}")
            for t in self.trade_history[-5:]:
                log.info(f"     {t['pair']}: {t['profit_pct']*100:+.2f}% (€{t['profit_eur']:+.2f}) "
                         f"[{t['reason'][:30]}]")
        log.info("═" * 60)


if __name__ == "__main__":
    bot = MomentumTrader()
    bot.run()
