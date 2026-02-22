# 🤖 AI Trading Bot - Quick Start

## ⚙️ Konfiguration

### 1. One Trading API Keys

Bearbeite `config.json`:
```json
"key": "DEIN_API_KEY",
"secret": "DEIN_API_SECRET"
```

### 2. LLM API Key (für intelligente Exits)

**Gemini (empfohlen, kostenlos):**
```bash
export GEMINI_API_KEY="your_key_here"
```

Oder in `~/.bashrc` / `~/.zshrc` eintragen!

**Alternativ Claude:**
```bash
export ANTHROPIC_API_KEY="your_key_here"
```

## 🚀 Bot starten

### Ohne LLM (Standard-Strategie):
```bash
cd ~/trading-bot
./freqtrade.sh trade --config config.json --strategy AggressiveScalper
```

### Mit LLM Intelligence (empfohlen):
```bash
export GEMINI_API_KEY="your_key"
./freqtrade.sh trade --config config.json --strategy AIAggressiveScalper
```

## 🎯 Wie LLM funktioniert

Der Bot nutzt **Gemini** oder **Claude**, um zu entscheiden:

**Situation:** Du bist bei +5% Gewinn
- **Ohne LLM**: Verkauft sofort bei 5% (ROI-Limit)
- **Mit LLM**: 
  - Prüft Trend-Stärke
  - Prüft Volumen
  - Prüft RSI/MACD
  - **Entscheidung**: "HOLD" wenn starker Trend → wartet auf +8%, +10%, etc.
  - **Oder**: "EXIT" wenn Momentum schwächer wird

**Beispiel:**
```
🤖 LLM: BTC/EUR | Profit: 5.2% | Action: HOLD | Confidence: 0.85
Reason: Strong upward momentum with increasing volume, RSI not overbought yet

🤖 LLM: ETH/EUR | Profit: 6.8% | Action: EXIT | Confidence: 0.92  
Reason: Volume declining and RSI showing early divergence, secure profits now
```

## 📊 Strategien-Vergleich

| Feature | AggressiveScalper | AIAggressiveScalper |
|---------|-------------------|---------------------|
| Exit bei 5% | Sofort | LLM entscheidet |
| Max Profit | ~5-6% | 8-12%+ möglich |
| Intelligenz | Algorithmus | KI-gestützt |
| API Kosten | Keine | ~0.01€/Tag |

## 🔧 LLM anpassen

In `AIAggressiveScalper.py`:
```python
llm_provider = "gemini"  # oder "claude"
llm_exit_confidence_threshold = 0.6  # 0.0-1.0
use_llm_for_exit = True
```

## ⚡ Live Trading (Echtes Geld sehen!)

1. **Dry Run deaktivieren:**
   In [config.json](file:///home/meo/trading-bot/config.json) Zeile 6:
   ```json
   "dry_run": false,
   ```

2. **Bot starten:**
   ```bash
   export GEMINI_API_KEY="..."
   cd ~/trading-bot
   ./freqtrade.sh trade --config config.json --strategy AIAggressiveScalper
   ```

3. **Jetzt siehst du deine echten 112 USDC!**

## 🆘 Troubleshooting

**LLM Error: API Key not found**
→ `export GEMINI_API_KEY=...` vergessen

**LLM Advisor not available**
→ `cd ~/trading-bot && PIP_USER=false ./venv/bin/pip install google-generativeai`

**Bot zu langsam**
→ Nutze `AggressiveScalper` ohne LLM
