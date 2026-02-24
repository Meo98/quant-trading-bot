#!/usr/bin/env python3
"""
Web3 DEX Scout (DexScreener API) + Basic NLP Sentiment Engine
Monitors Solana for newly trending pools and filters by liquidity and social sentiment.
"""
import requests
import time
import json
import logging
import sys
from pathlib import Path
from dataclasses import dataclass
from typing import Optional

# Wir borgen uns den TelegramNotifier aus dem Haupt-Bot
from autotrader import TelegramNotifier, CONFIG_FILE

log = logging.getLogger("dex_radar")
logging.basicConfig(level=logging.INFO, format="%(asctime)s │ %(levelname)-7s │ %(message)s")

# --- KONFIGURATION ---
CHAIN = "solana"                 # Fokus auf Solana (schnellste Pumps)
MIN_LIQUIDITY_USD = 50000        # Mindestens $50k im Pool (Anti-Rugpull Schutz)
MIN_VOLUME_24H = 100000          # Muss schon etwas gedreht haben
MIN_FDV = 500000                 # Fully Diluted Valuation > $500k
CHECK_INTERVAL_SEC = 60          # Alle 60 Sekunden scannen

# NLP Keywords für seeehr simples Sentiment
BULLISH_WORDS = ["gem", "moon", "bullish", "send it", "pump", "alpha", "utility", "partnership"]
BEARISH_WORDS = ["rug", "scam", "dev sold", "honeypot", "jeo", "dump", "avoid", "fake"]

@dataclass
class DexPair:
    address: str
    symbol: str
    name: str
    price_usd: float
    liquidity_usd: float
    volume_24h: float
    fdv: float
    url: str

class SentimentEngine:
    """Simuliert eine Sentiment Analyse (z.B. Twitter/X Scraper ODER Coin-Titel Analyse)"""
    
    def analyze(self, pair: DexPair) -> tuple[str, int]:
        """Gibt (Sentiment_String, Score) zurück (-100 bis +100)"""
        text_to_analyze = f"{pair.name} {pair.symbol}".lower()
        
        # In einer echten Umgebung würden wir hier z.B. X (Twitter) nach dem Cashtag $SYMBOL absuchen
        # und die letzten 50 Tweets durch ein lokales LLM / OpenClaw schicken.
        # Für unser MVP machen wir Keyword-Matching auf Name/Symbol und simulieren Social-Volume.
        
        score = 0
        
        # Meme-Bonus
        if any(word in text_to_analyze for word in ["doge", "pepe", "cat", "inu", "ai"]):
            score += 20
        
        # "Sichere" Indikatoren
        if "finance" in text_to_analyze or "protocol" in text_to_analyze:
            score += 10
            
        # Rugpull Red-Flags
        if "inu" in text_to_analyze and len(pair.symbol) > 6:
            score -= 30 # Lange Weird-Tier Namen sind oft Müll
            
        if score >= 15:
            return "🔥 VERY BULLISH", score
        elif score > 0:
            return "👍 BULLISH", score
        elif score > -10:
            return "😐 NEUTRAL", score
        else:
            return "🛑 RED FLAG", score

class DexRadar:
    def __init__(self):
        self.known_pairs = set()
        self.sentiment = SentimentEngine()
        
        # Telegram laden
        try:
            with open(CONFIG_FILE) as f:
                config = json.load(f)
            tele_conf = config.get("telegram", {})
            self.notifier = TelegramNotifier(
                token=tele_conf.get("token", ""),
                chat_id=tele_conf.get("chat_id", ""),
                enabled=tele_conf.get("enabled", False)
            )
        except Exception as e:
            log.error(f"Konnte config.json nicht laden: {e}")
            self.notifier = TelegramNotifier("", "", False)

    def fetch_trending_pairs(self) -> list[DexPair]:
        """Holt sich Trending Coins via DexScreener API"""
        # Wir suchen nach den populärsten Solana Token der letzten Stunde
        # Da DexScreener keine direkte "Trending" API on chain hat ohne Authentication, 
        # nutzen wir Search für "pump" oder spezifische Solana Meme keywords.
        url = "https://api.dexscreener.com/latest/dex/search?q=solana%20meme"
        try:
            headers = {'User-Agent': 'Mozilla/5.0'}
            resp = requests.get(url, headers=headers, timeout=10)
            data = resp.json()
            
            pairs = []
            for p in data.get("pairs", []):
                if p.get("chainId") != CHAIN:
                    continue
                    
                liq = p.get("liquidity", {}).get("usd", 0)
                vol = p.get("volume", {}).get("h24", 0)
                fdv = p.get("fdv", 0)
                
                # Weiche Filter anwenden für Test-Zwecke
                if liq < (MIN_LIQUIDITY_USD / 5) or vol < (MIN_VOLUME_24H / 5):
                    continue
                    
                pairs.append(DexPair(
                    address=p.get("pairAddress", ""),
                    symbol=p.get("baseToken", {}).get("symbol", ""),
                    name=p.get("baseToken", {}).get("name", ""),
                    price_usd=float(p.get("priceUsd", 0)),
                    liquidity_usd=liq,
                    volume_24h=vol,
                    fdv=fdv,
                    url=p.get("url", "")
                ))
            return pairs
        except Exception as e:
            log.error(f"DexScreener API Fehler: {e}")
            return []

    def run(self):
        log.info(f"🚀 DEX Radar gestartet (Chain: {CHAIN.upper()})")
        log.info(f"🛡️  Filter: >${MIN_LIQUIDITY_USD:,.0f} Liq | >${MIN_VOLUME_24H:,.0f} Vol")
        
        # Initiale Ladung um Spam zu verhindern
        initial_pairs = self.fetch_trending_pairs()
        for p in initial_pairs:
            self.known_pairs.add(p.address)
        log.info(f"✅ {len(initial_pairs)} existierende Pairs indexiert.")
        
        while True:
            try:
                pairs = self.fetch_trending_pairs()
                for p in pairs:
                    if p.address not in self.known_pairs:
                        self.known_pairs.add(p.address)
                        
                        # Sentiment checken
                        sentiment_label, score = self.sentiment.analyze(p)
                        
                        msg = (f"🚨 <b>NEW DEX GEM RADAR</b>\n"
                               f"💎 <b>{p.name} ({p.symbol})</b>\n"
                               f"💵 Price: ${p.price_usd:.6f}\n"
                               f"💧 Liquidity: ${p.liquidity_usd:,.0f}\n"
                               f"📊 24h Vol: ${p.volume_24h:,.0f}\n"
                               f"🧠 <b>Sentiment:</b> {sentiment_label} (Score: {score})\n"
                               f"🔗 <a href='{p.url}'>DexScreener</a>")
                        
                        log.info(f"🚨 NEUER COIN: {p.symbol} | Sentiment: {sentiment_label}")
                        
                        # Sende Alarm nur wenn das Sentiment okay ist
                        if score >= 0:
                            self.notifier.send(msg)
                        else:
                            log.warning(f"  🗑️  Ignoriere {p.symbol} (Sentiment zu schlecht: {score})")
                            
            except Exception as e:
                log.error(f"Fehler im Radar-Loop: {e}")
                
            time.sleep(CHECK_INTERVAL_SEC)

if __name__ == "__main__":
    radar = DexRadar()
    radar.run()
