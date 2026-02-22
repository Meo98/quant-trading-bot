import ccxt
import time
import json
import sys

# Load Config
try:
    with open('config.json') as f:
        config = json.load(f)
except Exception as e:
    sys.exit(1)

api = config.get('exchange', {})
key = api.get('key', '')
secret = api.get('secret', '')

print(f"Connecting to Kraken...")
exchange = ccxt.kraken({
    'apiKey': key,
    'secret': secret,
    'enableRateLimit': True,
})

pair = 'EUR/CHF' # We want to BUY EUR with CHF.

def swap():
    try:
        # 1. Fetch Balance (CHF)
        bal = exchange.fetch_balance()
        chf = bal['CHF']['free']
        
        print(f"Balance: {chf} CHF")
        
        if chf < 10:
            print("Balance too low (< 10 CHF).")
            return

        # 2. Fetch Price (EUR/CHF)
        ticker = exchange.fetch_ticker(pair)
        ask = ticker['ask'] # Lowest price to BUY EUR
        
        print(f"Price (EUR/CHF Ask): {ask}")
        
        # 3. Calculate Amount to Buy
        # We need to spend ALL CHF. 
        # Kraken charges fee ~0.26%.
        # Cost = Amount * Price.
        # Desired Cost = chf * 0.995 (reserve 0.5% for fees/buffer)
        
        target_cost_chf = chf * 0.995
        amount_eur = target_cost_chf / ask
        
        # Round down to 4 decimals?
        amount_eur = float(int(amount_eur * 10000) / 10000)
        
        print(f"Target: Spend {target_cost_chf:.2f} CHF -> Buy {amount_eur} EUR")
        
        # 4. Place MARKET BUY Order for EUR
        # (Kraken supports market buy with amount in base currency)
        
        print(f"Placing MARKET BUY order for {amount_eur} EUR...")
        
        order = exchange.create_order(
            symbol=pair,
            type='market',
            side='buy',
            amount=amount_eur
        )
        
        print(f"✅ Order Placed! ID: {order['id']}")
        
    except Exception as e:
        print(f"❌ Error: {e}")

if __name__ == "__main__":
    swap()
