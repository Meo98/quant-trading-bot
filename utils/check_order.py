import ccxt
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

exchange = ccxt.onetrading({
    'apiKey': key,
    'secret': secret,
    'enableRateLimit': True,
})

try:
    # 114186cc-d50b-4bfe-bea4-45cacc72f4db
    orders = exchange.fetch_open_orders()
    filled_orders = exchange.fetch_closed_orders(limit=1)
    
    print("\n🔵 OPEN ORDERS:")
    for o in orders:
        print(f"  {o['symbol']} {o['side']} {o['amount']} @ {o['price']} (Filled: {o['filled']})")
        
    print("\n✅ CLOSED ORDERS (Last 5):")
    for o in filled_orders:
        print(f"  {o['symbol']} {o['side']} {o['amount']} @ {o['average'] or o['price']} (Filled: {o['filled']})")
        
    # Check Balance again
    bal = exchange.fetch_balance()
    eur = bal['EUR']['free']
    print(f"\n💰 EUR Free: {eur:.2f} EUR")

except Exception as e:
    print(f"Error: {e}")
