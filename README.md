# Matrix Quant - Cryptocurrency Momentum Trading Bot

A high-performance cryptocurrency trading bot built with **Rust** and **Flutter**, designed for automated momentum trading on Kraken.

## Features

- **Pump Detection** - Identifies coins in early uptrends based on 24h/15m/1h momentum
- **Dynamic Trailing Stop** - Step-up trailing stops that widen as profits grow
- **Volatility Regime Selection** - Adjusts stop-loss levels based on coin volatility
- **Mobile-First** - Native Android/iOS apps with background trading
- **Biometric Security** - FaceID/Fingerprint authentication
- **Secure Storage** - API credentials encrypted on device

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Flutter Mobile App                       │
│                    (lib/main.dart)                          │
│  • Glassmorphism UI                                         │
│  • Biometric Auth                                           │
│  • Background Service                                       │
└────────────────────────┬────────────────────────────────────┘
                         │ flutter_rust_bridge
┌────────────────────────▼────────────────────────────────────┐
│                   Rust Core Engine                          │
│                   (rust/src/)                               │
│  • TradingEngine - Main trading loop                        │
│  • KrakenRestClient - API with HMAC-SHA512                  │
│  • Pump Detection & Exit Logic                              │
└─────────────────────────────────────────────────────────────┘
```

## Project Structure

```
trading-bot/
├── rust/                      # Rust trading engine
│   └── src/
│       ├── api/
│       │   ├── rest_client.rs # Kraken API client
│       │   └── simple.rs      # Flutter bridge functions
│       ├── trading/
│       │   ├── engine.rs      # Core trading logic
│       │   └── mod.rs         # OpenTrade struct
│       ├── config/mod.rs      # BotConfig
│       ├── lib.rs             # Library exports
│       └── main.rs            # Standalone daemon
├── lib/                       # Flutter app
│   └── main.dart              # Mobile UI
├── android/                   # Android platform
├── ios/                       # iOS platform
├── legacy_python_bot/         # Original Python implementation
│   ├── autotrader.py          # Python trading engine
│   ├── backtester.py          # Historical backtesting
│   └── dashboard.py           # Web dashboard
└── data/                      # Historical candle data
```

## Trading Strategy

### Entry Criteria (Pump Detection)

| Filter | Default | Description |
|--------|---------|-------------|
| Min 24h Change | 5% | Minimum pump to consider |
| Max 24h Change | 200% | Skip if already mooned |
| Min 15m Change | 1% | Short-term momentum |
| Min 1h Change | 2% | Medium-term momentum |
| Min Volume | €10,000 | Liquidity filter |
| Trend Filter | - | Skip falling prices |
| Cooldown | 30 min | After exit, wait before re-entry |

### Exit Logic (Trailing Stop)

```
Hard Stop-Loss: -15% (emergency exit)

Trailing Stop (dynamic):
  Base:       10% from peak
  At +20%:    15% from peak (STEP-UP)
  At +50%:    25% from peak (MOON-PHASE)

Volatility Adjustment:
  High (>40%):   15% trailing, -20% hard SL
  Medium (>15%): 10% trailing, -15% hard SL
  Low:            6% trailing, -10% hard SL
```

## Installation

### Prerequisites

- Rust 1.70+
- Flutter 3.10+
- Android SDK (for APK)
- Kraken API credentials

### Build Mobile App (Android)

```bash
# Install dependencies
flutter pub get

# Generate Rust bindings
flutter_rust_bridge_codegen generate

# Build APK
flutter build apk --release

# APK location: build/app/outputs/flutter-apk/app-release.apk
```

### Run Standalone Daemon (Linux/macOS)

```bash
cd rust
cargo build --release
./target/release/matrix-quant-core
```

## Configuration

Create `config.json` in the project root:

```json
{
  "max_open_trades": 3,
  "exchange": {
    "name": "kraken",
    "key": "YOUR_API_KEY",
    "secret": "YOUR_API_SECRET"
  },
  "dry_run": false
}
```

> **Security Note:** Never commit `config.json` to version control. It's in `.gitignore` by default.

### Mobile App Configuration

API credentials are entered in the Settings screen and stored encrypted using Flutter Secure Storage (Android Keystore / iOS Keychain).

## Security

### Credential Storage

| Platform | Method |
|----------|--------|
| Android | EncryptedSharedPreferences (AES-256) |
| iOS | Keychain Services |
| Desktop | config.json (user responsibility) |

### API Permissions Required

On Kraken, create an API key with these permissions only:
- Query Funds
- Query Open Orders & Trades
- Create & Modify Orders

**Do NOT enable:**
- Withdraw Funds
- Query Ledger Entries

### App Permissions (Android)

```xml
<uses-permission android:name="android.permission.INTERNET"/>
<uses-permission android:name="android.permission.USE_BIOMETRIC"/>
<uses-permission android:name="android.permission.FOREGROUND_SERVICE"/>
<uses-permission android:name="android.permission.WAKE_LOCK"/>
```

## Risk Warning

**This software is for educational purposes only.**

- Cryptocurrency trading involves substantial risk of loss
- Past performance does not guarantee future results
- Never trade with money you cannot afford to lose
- The developers are not responsible for any financial losses

## Development

### Running Tests

```bash
# Rust tests
cd rust && cargo test

# Flutter tests
flutter test
```

### Code Generation

After modifying Rust bridge functions:

```bash
flutter_rust_bridge_codegen generate
```

## License

MIT License - See [LICENSE](LICENSE) for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Commit your changes
4. Push to the branch
5. Create a Pull Request

---

Built with Rust + Flutter
