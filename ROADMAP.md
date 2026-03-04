# Matrix Quant - Feature Roadmap

## Phase 1: Interaktive Kontrolle

### 1.1 Interaktive Notifications
- Push-Benachrichtigung bei Trade-Signal: "BTCEUR Pump erkannt (+4.2%). Kaufen?"
- Action-Buttons: [Kaufen] [Ignorieren] [Blacklist]
- Notification bei Trailing-Stop Step-Up: "ETHEUR +8% - Stop-Loss auf +5% angehoben"
- Notification bei Sell: "SOLUSDT verkauft: +12.3% Gewinn"

### 1.2 Coin-Blacklist
- Liste von Coins die nie getradet werden sollen
- Persistent gespeichert
- Schnell hinzufuegen via Notification-Action oder Slash-Command
- Temporaere Blacklist (z.B. "DOGE fuer 24h ignorieren")

### 1.3 Manueller Trade-Kontrolle
- Trade sofort schliessen aus der App
- Alle Trades auf einen Blick mit Live-P&L
- Panic-Button: Alle Trades sofort schliessen

---

## Phase 2: Erweiterter Modus

### 2.1 Slash-Command Feld
Textfeld in der App mit `/` Befehlen:

```
/sell BTCEUR              - Trade sofort schliessen
/sell all                 - Alle Trades schliessen
/blacklist DOGE           - Coin auf Blacklist setzen
/whitelist DOGE           - Coin von Blacklist entfernen
/set trailing 8%          - Trailing-Stop aendern
/set stake 50             - Stake pro Trade auf 50 EUR
/set max_trades 5         - Max gleichzeitige Trades
/status                   - Aktuelle Trades + Balance
/history                  - Letzte 20 Trades
/pnl                      - Gesamtperformance heute/Woche/Monat
/pause                    - Trading pausieren
/resume                   - Trading fortsetzen
/ai <frage>               - AI-Assistent fragen
```

### 2.2 Erweiterte Einstellungen UI
- Trailing-Stop Prozent (Slider)
- Hard Stop-Loss Prozent (Slider)
- Step-Up Schwellen konfigurierbar
- Stake pro Trade (EUR)
- Max gleichzeitige Trades
- Min Pump-Staerke zum Einstieg
- Cooldown nach Trade
- Handelszeiten (z.B. nur 08:00-22:00)

---

## Phase 3: Integrierte Trading-AI

### 3.1 Konzept: Spezialisierte Trading-AI
Keine generische AI, sondern eine auf Krypto-Trading spezialisierte AI
die direkt in der App lebt.

### 3.2 Architektur-Optionen

**Option A: Cloud-basiert (Claude/OpenAI API)**
- AI API Key in App-Einstellungen hinterlegen
- Pro: Maechtig, aktuelles Wissen, grosse Kontextfenster
- Contra: Kosten pro Anfrage, Latenz, Internet noetig

**Option B: On-Device (lokales LLM)**
- Kleines quantisiertes Modell (z.B. Llama 3 8B GGUF) auf dem Handy
- Pro: Kostenlos, offline, keine Latenz, private Daten
- Contra: Begrenzte Intelligenz, braucht RAM, nur kleine Modelle

**Option C: Hybrid**
- Kleines lokales Modell fuer schnelle Entscheidungen
- Cloud-AI fuer komplexe Analysen (z.B. taeglicher Marktbericht)
- Bester Kompromiss

### 3.3 Was die AI koennen soll

**Trade-Bewertung (vor jedem Kauf):**
- Bekommt: Coin, Pump-Staerke, Volumen, Preis-Historie (24h), RSI, MACD
- Gibt zurueck: Score 0-100, Begruendung, empfohlener Trailing-Stop
- Bot kauft nur wenn Score > Schwellenwert (konfigurierbar)

**Marktanalyse:**
- Taeglicher Marktbericht (Trends, Sentiment, wichtige Events)
- Warnung bei ungewoehnlicher Marktlage (Flash Crash, extreme Volatilitaet)
- Korrelationsanalyse: "BTC faellt - deine Altcoin-Trades sind gefaehrdet"

**Chat-Interface:**
- Fragen zum Portfolio: "Wie laeuft mein ETHEUR Trade?"
- Strategie-Diskussion: "Sollte ich den Trailing-Stop enger setzen?"
- Erklaerungen: "Warum wurde SOLUSDT verkauft?"

**Lernfaehigkeit:**
- Analysiert vergangene Trades (was hat funktioniert, was nicht)
- Passt Empfehlungen an deinen Trading-Stil an
- "Deine besten Trades waren bei Pumps > 5% mit hohem Volumen"

### 3.4 Sicherheitsregeln fuer AI
- AI darf NIE autonom handeln ohne Bestaetigung (ausser im "Auto"-Modus)
- AI darf keine Funds transferieren (nur traden)
- Max Verlust pro AI-Empfehlung konfigurierbar
- Kill-Switch: AI-Trading sofort deaktivierbar
- Alle AI-Entscheidungen werden geloggt mit Begruendung

### 3.5 Datenquellen fuer AI-Kontext
- Kraken OHLCV Daten (Preis-Historie)
- Kraken Orderbook (Liquiditaet)
- Open Interest / Funding Rates (wenn verfuegbar)
- Eigene Trade-Historie (Performance-Daten)
- Optional: CoinGecko API fuer Markt-Sentiment, Social-Daten

---

## Phase 4: Zukunft (Ideen)

- Telegram-Bot Integration (Befehle + Notifications via Telegram)
- Web-Dashboard mit Charts
- Multi-Exchange Support (Binance, Coinbase)
- Paper-Trading Modus (kein echtes Geld, zum Testen)
- Portfolio-Tracker mit Gesamtuebersicht
- Steuer-Export (CSV fuer Krypto-Steuererklaerung)
- Widget auf Android Homescreen mit Live-P&L

---

## Priorisierung

1. **Jetzt**: Crash-Resilient Trading (v1.1.0) ✅
2. **Naechstes**: Phase 1 (Notifications + Blacklist + manuelle Kontrolle)
3. **Danach**: Phase 2 (Slash-Commands + erweiterte Settings)
4. **Spaeter**: Phase 3 (Trading-AI Integration)
