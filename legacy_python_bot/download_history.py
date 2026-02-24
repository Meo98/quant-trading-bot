#!/usr/bin/env python3
"""
Kraken Historical Data Downloader for Backtesting
Fetch 1-minute OHLCV candles for the most popular EUR pairs.
"""
import ccxt
import time
import json
import logging
import sys
from pathlib import Path
from datetime import datetime, timedelta, timezone

log_dir = Path(__file__).parent
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s │ %(message)s",
    datefmt="%H:%M:%S",
    handlers=[logging.StreamHandler(sys.stdout)]
)
log = logging.getLogger("downloader")

DATA_DIR = Path(__file__).parent / "data"
DATA_DIR.mkdir(exist_ok=True)

# We want 7 days of 1-minute data
DAYS = 7
TIMEFRAME = '1m'
CANDLES_PER_CALL = 720 # Kraken limit

def main():
    log.info("📡 Connecting to Kraken...")
    exchange = ccxt.kraken({
        'enableRateLimit': True,
        'rateLimit': 3000,
    })
    
    exchange.load_markets()
    
    # Get top active EUR pairs
    eur_pairs = [
        symbol for symbol, market in exchange.markets.items()
        if (market.get("quote") == "EUR"
            and market.get("active", True)
            and market.get("spot", True)
            and "/" in symbol
            and not symbol.endswith(".d"))
    ]
    
    # Sort pairs by 24h volume to get the most liquid ones
    log.info(f"📊 Found {len(eur_pairs)} EUR pairs. Filtering top 50 by volume...")
    
    top_pairs = ['BTC/EUR', 'ETH/EUR', 'SOL/EUR', 'DOGE/EUR', 'PEPE/EUR', 'WIF/EUR', 'BONK/EUR', 'FLOKI/EUR', 'SHIB/EUR']
    
    # Start fetching
    start_time = datetime.now(timezone.utc) - timedelta(days=DAYS)
    since_ms = int(start_time.timestamp() * 1000)
    end_ms = int(datetime.now(timezone.utc).timestamp() * 1000)
    
    log.info(f"📅 Downloading {DAYS} days of {TIMEFRAME} data starting from {start_time.strftime('%Y-%m-%d')}")
    
    for symbol in top_pairs:
        safe_sym = symbol.replace('/', '_')
        outfile = DATA_DIR / f"{safe_sym}_{TIMEFRAME}.json"
        
        log.info(f"⬇️ Downloading {symbol}...")
        all_candles = []
        current_since = since_ms
        
        try:
            while current_since < end_ms:
                candles = exchange.fetch_ohlcv(symbol, TIMEFRAME, since=current_since, limit=CANDLES_PER_CALL)
                if not candles:
                    break
                    
                all_candles.extend(candles)
                
                # Move pointer forward (last candle's timestamp + 1 minute)
                current_since = candles[-1][0] + 60000 
                time.sleep(exchange.rateLimit / 1000)
            
            # Save to disk
            with open(outfile, 'w') as f:
                json.dump(all_candles, f)
            log.info(f"✅ Saved {len(all_candles)} candles for {symbol} -> {outfile.name}")
            
        except Exception as e:
            log.error(f"❌ Failed to download {symbol}: {e}")

if __name__ == "__main__":
    main()
