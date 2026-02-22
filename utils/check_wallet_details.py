import requests
import json
import sys

try:
    with open('config.json') as f:
        config = json.load(f)
except Exception as e:
    sys.exit(1)

api_config = config.get('api_server', {})
base_url = f"http://127.0.0.1:{api_config.get('listen_port', 8080)}/api/v1"
auth = (api_config.get('username', 'freqtrader'), api_config.get('password', ''))

print(f"Connecting to Bot API...")

try:
    # Login
    resp = requests.post(f"{base_url}/token/login", auth=auth)
    if resp.status_code != 200:
        print(f"❌ Login Failed")
        sys.exit(1)
    
    token = resp.json()['access_token']
    headers = {'Authorization': f'Bearer {token}'}

    # Balance
    bal_data = requests.get(f"{base_url}/balance", headers=headers).json()
    
    print(f"\n💰 BOT WALLET OVERVIEW (USDC Value)")
    
    currencies = bal_data.get('currencies', [])
    # Support dict or list format
    if isinstance(currencies, dict):
        currencies = [v for k,v in currencies.items()]
        
    for c in currencies:
        # Check if currency, balance exists
        curr = c.get('currency', 'Unknown')
        bal = c.get('balance', 0)
        free = c.get('free', 0)
        used = c.get('used', 0)
        est = c.get('est_stake', 0)
        
        if bal > 0:
            print(f"  - {curr}: Total {bal:.4f} (Free: {free:.4f}, Locked in Orders: {used:.4f})")
            
    print(f"\n✅ Total Portfolio Value: {bal_data.get('total')} USDC")

except Exception as e:
    print(f"Error: {e}")
