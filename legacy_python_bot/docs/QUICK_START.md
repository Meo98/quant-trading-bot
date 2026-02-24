# ⚡ Quick Start Guide

## 1. Erste Einrichtung (3 Minuten)

### App installieren
1. APK auf dein Handy laden und installieren
2. App öffnen → **4-stellige PIN** setzen

### API Keys eingeben
1. Gehe zum Tab **Keys** 🔑
2. [Kraken API Key erstellen](https://www.kraken.com/u/security/api):
   - Permissions: **Query Funds** ✅ **Create Orders** ✅ **Query Orders** ✅
3. Key und Secret in die App eintragen
4. **💾 Save API Keys** drücken

### (Optional) Telegram Alarme
1. [@BotFather](https://t.me/BotFather) → `/newbot` → Token kopieren
2. [@userinfobot](https://t.me/userinfobot) → Chat ID kopieren
3. Beides in der App eingeben

---

## 2. Trading-Einstellungen verstehen

Gehe zum Tab **Strategy** ⚙️ — hier stellst du ein, wie aggressiv oder konservativ der Bot handelt.

### 🟢 Konservative Einstellungen (weniger Risiko)
```
Max Open Trades:     2        ← Weniger gleichzeitige Trades
Trailing Stop:       8%       ← Verkauft früher bei Kursrückgang
Hard Stop Loss:      10%      ← Engere Notbremse
Pump Detection:      10%      ← Nur starke Pumps kaufen
Min Volume:          50000    ← Nur liquide Coins
```

### 🔴 Aggressive Einstellungen (mehr Risiko, mehr Chancen)
```
Max Open Trades:     5        ← Mehr gleichzeitige Positionen
Trailing Stop:       15%      ← Lässt dem Pump mehr Platz
Hard Stop Loss:      20%      ← Größerer Spielraum
Pump Detection:      3%       ← Kauft auch kleinere Pumps
Min Volume:          5000     ← Auch kleinere Coins
```

### 🏁 Standard-Einstellungen (Empfohlen für Anfänger)
```
Max Open Trades:     3
Trailing Stop:       10%
Hard Stop Loss:      15%
Min Profit to Trail: 2.5%
Pump Detection:      5%
Min Volume:          10000
Cooldown:            30 min
Scan Interval:       5 sec
```

---

## 3. Bot starten

1. Gehe zum Tab **Bot** 📊
2. Drücke **▶ START BOT**
3. Beobachte die Live-Logs:
   - 🟢 Grün = Kauf / Pump erkannt
   - 🔴 Rot = Verkauf / Stop-Loss
   - 🟡 Gelb = Warnung

### Was du im Log siehst

```
18:05:02 │ ✅ Kraken verbunden | 582 EUR-Pairs
18:05:02 │ 💰 Balance: €45.23
18:05:07 │ 🚀 PUMP: ZEUS/EUR +47.8% | Vol: €13k | Vola: 50.7%
18:05:07 │ ✅ BUY ZEUS/EUR @ €0.0234 | Stake: €15.07
18:12:44 │ 📈 ZEUS/EUR +12.3% | High: +18.1% | Trailing...
18:15:02 │ 📉 TRAILING STOP ZEUS/EUR | Sold @ €0.0261 | +11.5% | €+1.73
```

---

## 4. Portfolio prüfen

1. Gehe zum Tab **Portfolio** 💰
2. Siehst dein gesamtes Kraken-Guthaben
3. Jeder Coin zeigt: Menge, EUR-Wert, 24h-Veränderung
4. Tippe 🔄 zum Aktualisieren

---

## 5. DEX Radar (Optional)

1. Gehe zum Tab **Radar** 🛰️
2. Drücke **▶ Start Radar**
3. Scannt Solana-DEXes nach neuen Meme-Coins
4. ⚠️ **Nur zur Info** — kauft NICHT automatisch!

---

## 6. Tipps

- **Starte mit Dry Run!** Schalte "Dry Run" in den Strategy-Einstellungen an. So siehst du, wie der Bot handeln würde — ohne echtes Geld.
- **Klein anfangen**: Starte mit €20-50 zum Testen.
- **Nicht anfassen**: Der Bot arbeitet am besten, wenn du ihn laufen lässt. Ständiges Ein-/Ausschalten schadet der Performance.
- **Telegram aktivieren**: So bekommst du Kauf/Verkauf-Alarme direkt aufs Handy, auch wenn die App im Hintergrund läuft.
