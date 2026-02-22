#!/usr/bin/env python3
"""Test OHLCV data fetching for One Trading"""

import ccxt
import time
from datetime import datetime, timedelta

print("Initializing One Trading...")
exchange = ccxt.onetrading()

pairs = ['BTC/USDC', 'ETH/USDC', 'SOL/USDC']

print("\nTesting OHLCV fetch (Simulating Bot Startup)...")

for pair in pairs:
    try:
        print(f"\nFetching {pair} 1m candles...")
        # Fetch last 1000 candles
        ohlcv = exchange.fetch_ohlcv(pair, '1m', limit=100)
        
        if ohlcv:
            last_candle = ohlcv[-1]
            last_time = datetime.fromtimestamp(last_candle[0] / 1000)
            print(f"✅ Success! Got {len(ohlcv)} candles.")
            print(f"   Last candle time: {last_time}")
            print(f"   Close price: {last_candle[4]}")
            
            # Check for outdated data
            now = datetime.now()
            diff = now - last_time
            if diff > timedelta(minutes=5):
                print(f"⚠️ WARNING: Data is outdated by {diff}!")
            else:
                print("   Data is fresh.")
        else:
            print("❌ No data returned!")
            
    except Exception as e:
        print(f"❌ Error fetching {pair}: {e}")
        
print("\nDone.")
