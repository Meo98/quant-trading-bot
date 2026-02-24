#!/usr/bin/env python3
"""
Kraken Momentum Offline Backtester
Simulates the autotrader.py logic over historical 1m candle data.
"""
import json
import logging
import sys
from pathlib import Path
from datetime import datetime
from autotrader import OpenTrade, MAX_OPEN_TRADES, TRAILING_STOP_PCT, HARD_STOP_LOSS_PCT, MIN_PROFIT_TO_EXIT

log = logging.getLogger("backtester")
logging.basicConfig(level=logging.INFO, format="%(asctime)s │ %(message)s")

DATA_DIR = Path(__file__).parent / "data"

class Backtester:
    def __init__(self, initial_balance=40.0):
        self.eur_balance = initial_balance
        self.open_trades: dict[str, OpenTrade] = {}
        self.trade_history: list[dict] = []
        self.total_profit = 0.0
        
    def run_simulation(self):
        # 1. Daten laden
        market_data = {}
        for file in DATA_DIR.glob("*.json"):
            symbol = file.name.replace("_1m.json", "").replace("_", "/")
            with open(file) as f:
                candles = json.load(f)
                # wandle in dict: timestamp -> (open, high, low, close, vol)
                market_data[symbol] = {c[0]: c for c in candles}
                
        if not market_data:
            log.error("❌ Keine Daten im /data Ordner gefunden. Bitte erst download_history.py ausführen!")
            return

        # Sammle alle Timestamps (sortiert)
        all_timestamps = set()
        for symbol, data in market_data.items():
            all_timestamps.update(data.keys())
            
        timeline = sorted(list(all_timestamps))
        
        log.info(f"🚀 Starte Backtest. Balance: €{self.eur_balance:.2f} | Datensätze: {len(timeline)} Minuten")
        log.info(f"📈 Pairs: {list(market_data.keys())}")
        
        for ts in timeline:
            curr_time = datetime.fromtimestamp(ts/1000)
            
            # --- 1. Exit Check für offene Trades ---
            pairs_to_sell = []
            for pair, trade in list(self.open_trades.items()):
                if ts not in market_data.get(pair, {}):
                    continue
                    
                candle = market_data[pair][ts]
                close_price = candle[4]
                high_price = candle[2]
                low_price = candle[3]
                
                # Update High
                if high_price > trade.highest_price:
                    trade.highest_price = high_price
                    
                # Simuliere intra-candle Stop-Loss
                # Wenn das low_price unter unseren Stop fällt, fliegen wir raus
                profit_at_low = trade.profit_pct(low_price)
                
                # Check Hard Stop
                if profit_at_low <= trade.hard_sl_pct:
                    pairs_to_sell.append((pair, "🛑 HARD STOP", trade.entry_price * (1 + trade.hard_sl_pct)))
                    continue
                    
                # Check Trailing Stop (am Ende der Minute der Einfachheit halber)
                profit_close = trade.profit_pct(close_price)
                if profit_close > MIN_PROFIT_TO_EXIT:
                    drawdown = trade.drawdown_from_high(close_price)
                    if drawdown <= -trade.trailing_stop_pct:
                        pairs_to_sell.append((pair, "📉 TRAILING STOP", close_price))
                        continue
                        
            # Execute Sells
            for pair, reason, exit_price in pairs_to_sell:
                trade = self.open_trades.pop(pair)
                profit_eur = trade.profit_eur(exit_price)
                profit_pct = trade.profit_pct(exit_price)
                self.eur_balance += trade.stake_eur + profit_eur
                self.total_profit += profit_eur
                
                self.trade_history.append({
                    "time": curr_time.strftime("%m-%d %H:%M"),
                    "pair": pair,
                    "profit_pct": profit_pct,
                    "profit_eur": profit_eur,
                    "reason": reason
                })
                log.info(f"{curr_time.strftime('%H:%M')} | SELL {pair} {profit_pct*100:+.1f}% (€{profit_eur:+.2f})")
                
            # --- 2. Buy Logic (Simulierter Pump Radar) ---
            if len(self.open_trades) < MAX_OPEN_TRADES:
                # Suche nach Coins die GERADE (in dieser 1 Minute) gepumpt haben!
                for pair, data in market_data.items():
                    if pair in self.open_trades:
                        continue
                        
                    if ts not in data:
                        continue
                        
                    candle = data[ts]
                    open_price = candle[1]
                    close_price = candle[4]
                    
                    # Simpler Backtest 1-Minute Pump Detect:
                    # Wenn der Coin in einer Minute > 0.1% steigt -> KAUFEN (für Majors)
                    if open_price > 0 and close_price > open_price:
                        min_pump = (close_price - open_price) / open_price
                        
                        if min_pump > 0.001: # 0.1% in 60 sekunden (Test)
                            # KAUFEN
                            stake = self.eur_balance / (MAX_OPEN_TRADES - len(self.open_trades))
                            if stake < 5: continue
                            
                            self.eur_balance -= stake
                            self.open_trades[pair] = OpenTrade(
                                pair=pair, entry_price=close_price, amount=stake/close_price,
                                stake_eur=stake, entry_time=curr_time,
                                highest_price=close_price, order_id="BT",
                                trailing_stop_pct=0.08, hard_sl_pct=-0.15 # statisch für BT
                            )
                            log.info(f"{curr_time.strftime('%H:%M')} | BUY {pair} @ {close_price:.4f} (Pump: {min_pump*100:.1f}%)")
                            
                            if len(self.open_trades) >= MAX_OPEN_TRADES:
                                break
                                
        # ENDE: Liquidate remaining open trades for final P/L calculation
        for pair, trade in list(self.open_trades.items()):
            last_candle = timeline[-1]
            if pair in market_data and last_candle in market_data[pair]:
                close_price = market_data[pair][last_candle][4]
                profit_eur = trade.profit_eur(close_price)
                profit_pct = trade.profit_pct(close_price)
                self.eur_balance += trade.stake_eur + profit_eur
                self.total_profit += profit_eur
                self.trade_history.append({
                    "time": "END", "pair": pair, "profit_pct": profit_pct,
                    "profit_eur": profit_eur, "reason": "🔚 BACKTEST END LIQUIDATION"
                })
                log.info(f"END   | SELL {pair} {profit_pct*100:+.1f}% (€{profit_eur:+.2f})")
                
        self._print_stats()

    def _print_stats(self):
        log.info("="*50)
        log.info("🏁 BACKTEST BEENDET")
        log.info(f"💰 End-Balance: €{self.eur_balance:.2f} (Start: €40.00)")
        log.info(f"💶 Net Profit: €{self.total_profit:+.2f}")
        log.info(f"📊 Total Trades: {len(self.trade_history)}")
        
        if self.trade_history:
            wins = sum(1 for t in self.trade_history if t["profit_pct"] > 0)
            losses = len(self.trade_history) - wins
            log.info(f"🏆 Win-Rate: {(wins/len(self.trade_history))*100:.1f}% ({wins} W / {losses} L)")
            
            best = max(self.trade_history, key=lambda x: x["profit_pct"])
            worst = min(self.trade_history, key=lambda x: x["profit_pct"])
            log.info(f"🚀 Bester Trade:  {best['pair']} {best['profit_pct']*100:+.1f}%")
            log.info(f"💥 Schlechtester: {worst['pair']} {worst['profit_pct']*100:+.1f}%")
        log.info("="*50)

if __name__ == "__main__":
    Backtester().run_simulation()
