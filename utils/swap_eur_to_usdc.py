#!/usr/bin/env python3
"""Swap EUR to USDC on One Trading (Limit Order Version)"""

import ccxt
import json
import sys

# Load config
try:
    with open('config.json', 'r') as f:
        config = json.load(f)
        api_key = config['exchange']['key']
        secret = config['exchange']['secret']
except Exception as e:
    print(f"Error loading config.json: {e}")
    sys.exit(1)

print(f"Connecting to One Trading...")
exchange = ccxt.onetrading({
    'apiKey': api_key,
    'secret': secret,
})

try:
    print("Loading markets to get precision info...")
    exchange.load_markets()
    
    # 1. Check Balance
    balance = exchange.fetch_balance()
    eur_balance = balance['free']['EUR']
    print(f"Current EUR Balance: {eur_balance} EUR")
    
    if eur_balance < 10:
        print("❌ Not enough EUR to swap (need at least 10 EUR)")
        sys.exit(1)

    # 2. Get Best Ask Price
    print("Fetching order book for USDC/EUR...")
    order_book = exchange.fetch_order_book('USDC/EUR')
    best_ask = order_book['asks'][0][0]
    print(f"Best Ask Price: {best_ask}")
    
    # 3. Calculate Limit Order Parameters
    # We buy at a price slightly higher than ask to ensure fill (acting like market)
    limit_price = best_ask * 1.02 # 2% buffer
    
    # Calculate amount: Spend (Balance - 1 EUR)
    spend_eur = eur_balance - 1.0
    amount_usdc = spend_eur / limit_price
    
    # IMPORTANT: Round to correct precision
    symbol = 'USDC/EUR'
    amount_usdc_prec = exchange.amount_to_precision(symbol, amount_usdc)
    price_prec = exchange.price_to_precision(symbol, limit_price)
    
    print(f"Planning Limit Buy:")
    print(f"  Price: {price_prec} EUR")
    print(f"  Amount: {amount_usdc_prec} USDC")
    print(f"  Est. Cost: {float(amount_usdc_prec) * float(price_prec)} EUR")
    
    # 4. Execute Trade (Limit Order)
    print("Placing LIMIT BUY order...")
    # Using ImmediateOrCancel (IOC) to avoid stuck orders, or just standard GTC
    # Let's use standard GTC (Good Till Cancelled)
    order = exchange.create_limit_buy_order(symbol, amount_usdc_prec, price_prec)
    
    print("\n✅ ORDER PLACED!")
    print(f"Order ID: {order['id']}")
    print(f"Status: {order['status']}")
    
    # 5. Show New Balance
    # Wait a moment for fill?
    print("Checking balance...")
    new_balance = exchange.fetch_balance()
    print(f"  USDC: {new_balance['total'].get('USDC', 0)}")
    print(f"  EUR: {new_balance['total'].get('EUR', 0)}")

except Exception as e:
    print(f"\n❌ SWAP FAILED: {e}")
