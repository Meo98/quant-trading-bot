import requests
import json
import sys
import time

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
    for i in range(3):
        try:
            resp = requests.post(f"{base_url}/token/login", auth=auth)
            if resp.status_code == 200: break
        except: pass
        time.sleep(1)
    
    if 'resp' not in locals() or resp.status_code != 200:
        print(f"❌ Login Failed")
        sys.exit(1)
    
    token = resp.json()['access_token']
    headers = {'Authorization': f'Bearer {token}'}

    # Trades
    print("Fetching trades...")
    trades_resp = requests.get(f"{base_url}/trades", headers=headers).json()
    trades = trades_resp.get('trades', [])
    
    print(f"\n📈 TRADING HISTORY ({len(trades)} trades):")
    for t in trades:
        pair = t.get('pair')
        profit_pct = t.get('profit_ratio', 0) * 100
        profit_abs = t.get('profit_abs', 0)
        direction = "LONG" if not t.get('is_short') else "SHORT"
        
        if t.get('is_open'):
            print(f"  🔵 OPEN {direction}: {pair} | Profit: {profit_pct:.2f}% | {profit_abs:.2f} USDC")
        else:
            print(f"  ✅ CLOSED {direction}: {pair} | Profit: {profit_pct:.2f}% | {profit_abs:.2f} USDC")
            
    if not trades:
        print("  No trades executed yet.")
        
    # Check Logs for warnings about outdated data
    print("\n🔍 Checking for Data Issues in Logs:")
    logs = requests.get(f"{base_url}/logs", headers=headers).json()
    issues_found = False
    for log in logs[-20:]:
        msg = log[4]
        if "Outdated history" in msg or "Missing data" in msg:
            print(f"  ⚠️  {msg}")
            issues_found = True
            
    if not issues_found:
        print("  No recent data warnings found.")

except Exception as e:
    print(f"Error: {e}")
