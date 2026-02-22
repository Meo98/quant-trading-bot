#!/usr/bin/env python3
"""Check balances on One Trading"""

import ccxt
import json
import sys

# Load config to get API keys
try:
    with open('config.json', 'r') as f:
        config = json.load(f)
        api_key = config['exchange']['key']
        secret = config['exchange']['secret']
except Exception as e:
    print(f"Error loading config.json: {e}")
    sys.exit(1)

if not secret:
    # Try to use the secret from the previous step if it was empty in the file
    # But wait, looking at the user edits, the secret IS empty in the file!
    # The user posted: "secret": "",
    # I suspect the user might not have entered the secret properly or I missed it.
    # Ah, I see the user edited config.json in Step 234 and put a very long string in "key" but "secret" is empty.
    # One Trading API keys usually have a separate secret. 
    # BUT, the token shown in "key" looks like a JWT or a long bearer token.
    # Let's check ccxt one trading auth.
    pass

print(f"Using API Key: {api_key[:10]}...")

try:
    exchange = ccxt.onetrading({
        'apiKey': api_key,
        'secret': secret,
    })
    
    # One Trading might use a different auth structure or the user pasted the wrong thing.
    # Let's try to fetch balance.
    balance = exchange.fetch_balance()
    
    print("\n💰 WALLET BALANCES:")
    found_funds = False
    for currency, amount in balance['total'].items():
        if amount > 0:
            found_funds = True
            free = balance['free'].get(currency, 0)
            used = balance['used'].get(currency, 0)
            print(f"  {currency}: {amount} (Free: {free}, Used: {used})")
            
    if not found_funds:
        print("  No funds found (Balance is 0)")

except Exception as e:
    print(f"\n❌ ERROR: Could not connect to exchange.")
    print(f"Details: {e}")
    print("\nPlease check if your API Key and Secret are correct in config.json!")
