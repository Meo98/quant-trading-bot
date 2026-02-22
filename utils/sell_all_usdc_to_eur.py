import ccxt
import time
import json
import sys

# Load Config
try:
    with open('config.json') as f:
        config = json.load(f)
except Exception as e:
    print(f"Error loading config: {e}")
    sys.exit(1)

# Initialize Exchange
api = config.get('exchange', {})
key = api.get('key', '')
secret = api.get('secret', '')

print(f"Connecting to One Trading with key ending in ...{key[-4:] if len(key) > 4 else key}")

exchange = ccxt.onetrading({
    'apiKey': key,
    'secret': secret,
    'enableRateLimit': True,
})

# Market Symbol
pair = 'USDC/EUR' 

def sell_all():
    try:
        # 1. Fetch Balance
        print("Fetching balance...")
        balance = exchange.fetch_balance()
        usdc_free = balance['USDC']['free']
        
        print(f"USDC Available: {usdc_free}")
        
        if usdc_free < 1.0: # Minimum trade size check
            print("Balance too low to trade.")
            return

        # 2. Fetch Order Book for Price
        print(f"Fetching order book for {pair}...")
        order_book = exchange.fetch_order_book(pair)
        # Sell -> match highest BID
        best_bid = order_book['bids'][0][0]
        
        print(f"Best Bid Price: {best_bid} EUR")
        
        # 3. Place Limit Sell Order
        # Use slightly lower price to ensure fill (buffer)? 
        # Or just use best bid. 
        # For 'market' behavior with limit order, use 0.5% lower price.
        sell_price = best_bid * 0.995 
        
        print(f"Placing LIMIT SELL order for {usdc_free} USDC at {sell_price:.4f} EUR...")
        
        order = exchange.create_order(
            symbol=pair,
            type='limit',
            side='sell',
            amount=usdc_free,
            price=sell_price
        )
        
        print("✅ Order Placed Successfully!")
        print(f"Order ID: {order['id']}")
        print(f"Status: {order['status']}")
        
    except Exception as e:
        print(f"❌ Error: {e}")

if __name__ == "__main__":
    sell_all()
