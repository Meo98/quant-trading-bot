import requests
import json
import sys
import time

try:
    with open('config.json') as f:
        config = json.load(f)
except Exception as e:
    print(f"Error loading config: {e}")
    sys.exit(1)

api_config = config.get('api_server', {})
base_url = f"http://127.0.0.1:{api_config.get('listen_port', 8080)}/api/v1"
auth = (api_config.get('username', 'freqtrader'), api_config.get('password', ''))

print(f"Checking API at {base_url}...")

# Retry loop for initial connection
for i in range(5):
    try:
        resp = requests.post(f"{base_url}/token/login", auth=auth)
        if resp.status_code == 200:
            break
    except:
        pass
    time.sleep(1)
    print(".", end="", flush=True)

try:
    if 'resp' not in locals() or resp.status_code != 200:
        print(f"\n❌ Login Failed: Bot not ready yet?")
        sys.exit(1)
    
    token = resp.json()['access_token']
    headers = {'Authorization': f'Bearer {token}'}
    
    # Status
    print("\nFetching status...")
    status_resp = requests.get(f"{base_url}/status", headers=headers)
    status_data = status_resp.json()
    
    if isinstance(status_data, dict):
        print(f"✅ Bot Status: {status_data.get('status')} ({status_data.get('state')})")
    else:
        print(f"⚠️ Status returned unexpected format: {status_data}")

    # Balance
    print("Fetching balance...")
    bal_data = requests.get(f"{base_url}/balance", headers=headers).json()
    if isinstance(bal_data, dict):
        total = bal_data.get('total', 0)
        sym = bal_data.get('symbol', 'USDC')
        print(f"💰 Balance: {total} {sym}")
        
        # Details
        for coin, data in bal_data.get('currencies', {}).items():
            if data['balance'] > 0:
                print(f"   - {coin}: {data['balance']}")
    
    # Logs (Last 3)
    print("Fetching logs...")
    logs = requests.get(f"{base_url}/logs", headers=headers).json()
    if isinstance(logs, list):
        print("\n📜 Recent Logs:")
        for log in logs[-3:]:
             print(f"  {log[4]}")

except Exception as e:
    print(f"\n❌ info fetch error: {e}")
