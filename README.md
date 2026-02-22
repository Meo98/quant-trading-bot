# 🚀 Matrix Quant Trader

A momentum trading bot for Kraken that detects pumping coins and rides the wave with trailing stops. Features a 5-tab mobile app (Android APK), Telegram alerts, Solana DEX scouting, and configurable risk management.

## 📱 The App

The Matrix Quant Trader is a full **Android app** built with [Flet](https://flet.dev) (Python → Flutter). It has 5 tabs:

| Tab | What it does |
|-----|-------------|
| **Bot** | Start/stop the trading bot, live log viewer |
| **Portfolio** | Live Kraken balance with 24h% change per asset |
| **Radar** | Solana DEX scout — finds new trending meme coins |
| **Strategy** | Configure all trading parameters (see below) |
| **Keys** | Securely store API keys (local only, never uploaded) |

### Security
- **PIN Lock** on every app start (4+ digits)
- **Fingerprint/Face Unlock**: Use your phone's built-in App Lock feature (Samsung, Xiaomi, OnePlus, Pixel all support this)
- API keys stored locally in `.app_secrets.json` (git-ignored)

## ⚙️ Strategy Settings

All trading parameters are configurable in the app. Here's what each one does:

### Trade Management

| Setting | Default | What it does |
|---------|---------|-------------|
| **Max Open Trades** | 3 slots | How many coins the bot can hold at the same time. More = more risk, more chances. |
| **Trailing Stop** | 10% | When a coin you bought drops this % from its **highest price**, the bot sells. This locks in profits when momentum fades. Lower = safer but exits earlier. |
| **Hard Stop Loss** | 15% | Emergency exit: if a coin drops this % from your **buy price**, sell immediately. This is your safety net against crashes. |
| **Min Profit to Trail** | 2.5% | The trailing stop only activates after this minimum profit %. Prevents the bot from selling too early on small dips. |

### Pump Detection

| Setting | Default | What it does |
|---------|---------|-------------|
| **Pump Detection** | 5% (24h) | Minimum 24-hour price increase to consider a coin as "pumping". Lower = more trades (riskier), higher = more selective. |
| **Min Volume** | €10,000 | Minimum 24h trading volume in EUR. Filters out low-liquidity scam/dead coins that could trap you. |
| **Pump Cooldown** | 30 min | After selling a coin, wait this many minutes before buying it again. Prevents getting caught in the same dump twice. |
| **Scan Interval** | 5 sec | How often the bot checks all 582 EUR pairs on Kraken. Lower = faster reaction but more API calls. |

### Mode

| Setting | Default | What it does |
|---------|---------|-------------|
| **Dry Run** | Off | **Simulation mode**: the bot runs exactly like normal but never places real orders. Perfect for testing your settings risk-free! |

### How the Bot Trades (Step by Step)

```
1. SCAN    → Checks all 582 EUR pairs on Kraken every 5 seconds
2. DETECT  → Finds coins pumping >5% in 24h with >€10k volume
3. CHECK   → Verifies market health (BTC/ETH not crashing)
4. BUY     → Places a buy order (splits balance across open slots)
5. MONITOR → Tracks the highest price after buying
6. SELL     → Trailing stop: sells when price drops 10% from peak
             Hard stop: sells immediately if down 15% from buy
```

## 🔧 Installation

### Option A: Android App (Recommended)
1. Download the APK from [GitHub Releases](https://github.com/Meo98/quant-trading-bot/releases) or build it yourself (`flet build apk`)
2. Install on your Android phone
3. Set a PIN → Enter API Keys → Configure Strategy → Start Bot

### Option B: Desktop (Python)
```bash
git clone https://github.com/Meo98/quant-trading-bot.git
cd quant-trading-bot
python3 -m venv venv
source venv/bin/activate
pip install ccxt flet flet-desktop-light requests

# Run the app in browser mode
python main.py

# Or run just the trading bot (headless)
python autotrader.py
```

### Kraken API Keys
1. Go to [kraken.com/u/security/api](https://www.kraken.com/u/security/api)
2. Create a new API key with permissions: **Query Funds**, **Create & Modify Orders**, **Query Open Orders**
3. Enter the Key and Secret in the app's **Keys** tab

### Telegram Alerts (Optional)
1. Message [@BotFather](https://t.me/BotFather) on Telegram → `/newbot` → get your token
2. Message [@userinfobot](https://t.me/userinfobot) → get your chat ID
3. Enter both in the app's **Keys** tab

## 📊 Additional Tools

| Tool | Command | What it does |
|------|---------|-------------|
| **Web Dashboard** | `python dashboard.py` | Flask-based web UI at localhost:5000 |
| **Backtester** | `python backtester.py` | Simulate trading strategy on historical data |
| **History Download** | `python download_history.py` | Download 7 days of 1-minute candles from Kraken |
| **DEX Radar** | `python dex_radar.py` | Standalone Solana DEX scanner with Telegram alerts |

## 🛡️ Risk Management Built-In

The bot has multiple layers of protection:

1. **Trailing Stop Loss** — Locks in profits when momentum fades
2. **Hard Stop Loss** — Emergency exit if things go very wrong
3. **Circuit Breakers** — Pauses buying if BTC/ETH crash or too many coins are red
4. **Volume Filter** — Ignores low-liquidity trap coins
5. **Cooldown Timer** — Prevents re-buying coins that just dumped
6. **Exchange-Side Stop Loss** — Stop orders placed directly on Kraken's servers (work even if bot crashes)

## ⚠️ Disclaimer

This bot is for **educational purposes only**. Cryptocurrency trading is extremely risky. Never trade with money you cannot afford to lose. The developers are not responsible for any financial losses.
