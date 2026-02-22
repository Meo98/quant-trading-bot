# 🚀 Quant System AutoTrader (MVP)

A highly automated, dynamic momentum trading bot designed for Kraken, focusing on highly volatile EUR pairs. This project features a full command center UI, Telegram integration, regime-based dynamic risk management, and a Solana DEX scouting radar.

## ✨ Features
- **Dynamic Regimes:** Calculates coin-specific volatility before entering a trade to set personalized trailing stops for either extreme "Memecoin" conditions or calmer "Altcoin" markets.
- **Circuit Breakers:** Scans global market health (BTC/ETH trends and Red/Green ratios). If the market goes toxic, the bot engages a "cooldown" phase.
- **Offline Backtester:** Simulate your exact trading logic locally using historical Kraken data downloaded to your machine.
- **Web Dashboard:** A Flask-powered "Glassmorphism" UI to monitor active slots, live terminal logs, and live configuration updates.
- **DEX Radar (Web3 Scout):** Runs in parallel to monitor the Solana blockchain via DexScreener for newly launched >$50k liquidity gems.

## 📦 Installation & Setup

1. **Clone the repository:**
   ```bash
   git clone https://github.com/Meo98/quant-trading-bot.git
   cd quant-trading-bot
   ```

2. **Set up the Python Virtual Environment:**
   ```bash
   python3 -m venv venv
   source venv/bin/activate
   pip install -r requirements.txt
   ```

3. **Configuration:**
   Rename `config.example.json` to `config.json` (this file is ignored by git for security) and enter your Kraken API keys and Telegram Bot token.

4. **Running the Bot & Dashboard:**
   ```bash
   # Start the Trading Bot
   python3 autotrader.py
   
   # Start the Command Center Dashboard
   python3 dashboard.py
   ```

## 📱 Mobile App (APK) Project
To transform this into an Android APK, see `build_apk.md` for instructions on wrapping the Python logic into `Flet` or `Kivy`.

## ⚠️ Disclaimer
This bot is for educational purposes only. Cryptocurrency trading is incredibly risky. Never trade with money you cannot afford to lose. The developers are not responsible for any financial losses.
