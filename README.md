# Matrix Quant - Cryptocurrency Momentum Trading Bot

A high-performance cryptocurrency trading bot built with **Rust** and **Flutter**, designed for automated momentum trading on the Kraken exchange. Runs on Android, iOS, Linux, Windows, and macOS.

## Features

- **Pump Detection** - Identifies coins in early uptrends based on 24h/15m/1h momentum filters
- **Dynamic Trailing Stop** - Step-up trailing stops that widen as profits grow (base, step-up, moon-phase)
- **Volatility Regime Selection** - Automatically adjusts stop-loss levels based on coin volatility
- **Strategy Presets** - Conservative, Moderate, Aggressive, or fully Custom strategies with German-language help texts
- **Mobile-First** - Native Android/iOS app with 24/7 background trading via Foreground Service
- **Desktop Support** - Identical UI on Linux, Windows, macOS (Flutter cross-platform)
- **Biometric Security** - FaceID/Fingerprint authentication via `local_auth`
- **Encrypted Storage** - API credentials stored via AES-256 (Android Keystore / iOS Keychain)
- **Network Hardening** - HTTPS-only, cleartext traffic blocked, ProGuard/R8 code obfuscation
- **Standalone Daemon** - Run headless on a server via `cargo build --release`

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                Flutter App (Android / Desktop)              │
│                    (lib/main.dart)                          │
│  • Glassmorphism UI with bottom navigation                  │
│  • Strategy presets (Konservativ/Moderat/Aggressiv/Custom)  │
│  • Biometric Auth + Secure Storage                          │
│  • Background Service (60s tick interval)                   │
└────────────────────────┬────────────────────────────────────┘
                         │ flutter_rust_bridge (v2.11.1)
┌────────────────────────▼────────────────────────────────────┐
│                   Rust Core Engine                          │
│                   (rust/src/)                               │
│  • TradingEngine  – Pump detection + trailing stop exits    │
│  • KrakenRestClient – REST API with HMAC-SHA512 signing     │
│  • Bridge DTOs – EngineStatusDto, ConfigDto, TradeDto       │
└─────────────────────────────────────────────────────────────┘
```

## Project Structure

```
trading-bot/
├── rust/                          # Rust trading engine
│   └── src/
│       ├── api/
│       │   ├── rest_client.rs     # Kraken REST client (HMAC-SHA512)
│       │   └── simple.rs          # Flutter bridge functions & DTOs
│       ├── trading/
│       │   ├── engine.rs          # Core trading logic (734+ lines)
│       │   └── mod.rs             # OpenTrade / TradeInfo structs
│       ├── config/mod.rs          # BotConfig
│       ├── lib.rs                 # Library exports
│       └── main.rs                # Standalone daemon (config.json)
├── lib/
│   └── main.dart                  # Flutter app (all screens)
├── android/
│   └── app/
│       ├── build.gradle.kts       # ProGuard, minify, shrink
│       ├── proguard-rules.pro     # Custom keep rules
│       └── src/main/
│           ├── AndroidManifest.xml # Permissions, services
│           └── res/xml/
│               └── network_security_config.xml
├── ios/                           # iOS platform
├── legacy_python_bot/             # Original Python bot
│   ├── autotrader.py
│   ├── backtester.py
│   └── dashboard.py
├── data/                          # Historical candle data
├── config.example.json            # Safe config template
├── SECURITY.md                    # Security audit report
└── BUILD.md                       # Build instructions (all platforms)
```

## Trading Strategy

### Strategy Presets

| Preset | Risk | Max Trades | Hard SL | Trailing | Best For |
|--------|------|------------|---------|----------|----------|
| Konservativ | Low | 2 | -8% | 6% | Beginners |
| Moderat | Medium | 3 | -15% | 10% | Standard |
| Aggressiv | High | 5 | -20% | 15% | Experienced |
| Custom | - | - | - | - | Full control |

### Entry Criteria (Pump Detection)

| Filter | Default | Description |
|--------|---------|-------------|
| Min 24h Change | 5% | Minimum pump to consider |
| Max 24h Change | 200% | Skip if already mooned |
| Min 15m Change | 1% | Short-term momentum confirmation |
| Min 1h Change | 2% | Medium-term momentum confirmation |
| Min Volume | 10,000 EUR | Liquidity filter |
| Trend Filter | Enabled | Skip falling prices |
| Cooldown | 30 min | Wait after exit before re-entry |

### Exit Logic (Dynamic Trailing Stop)

```
Hard Stop-Loss: -15% (emergency exit)

Trailing Stop (step-up):
  Base:       10% from peak
  At +20%:    15% from peak (STEP-UP)
  At +50%:    25% from peak (MOON-PHASE)

Volatility Adjustment:
  Hyper-volatile (>40%):  15% trailing, -20% hard SL
  Volatile (>15%):        10% trailing, -15% hard SL
  Calm (<15%):             6% trailing, -10% hard SL
```

## Quick Start

### Prerequisites

- Rust 1.70+ (`rustup.rs`)
- Flutter 3.10+ (`flutter.dev`)
- Android SDK 33+ (for APK)
- Kraken API credentials

### Build Android APK

```bash
flutter pub get
flutter_rust_bridge_codegen generate
flutter build apk --release --obfuscate --split-debug-info=build/debug-info
# Output: build/app/outputs/flutter-apk/app-release.apk
```

### Build Desktop (Linux/Windows/macOS)

```bash
flutter config --enable-linux-desktop    # or --enable-windows-desktop / --enable-macos-desktop
flutter build linux --release
# Output: build/linux/x64/release/bundle/
```

### Run Standalone Daemon

```bash
cd rust
cp ../config.example.json ../config.json
# Edit config.json with your API keys
cargo build --release
./target/release/matrix-quant-core
```

See [BUILD.md](BUILD.md) for detailed build instructions, release signing, AppImage creation, and systemd service setup.

## Configuration

### Mobile App

API credentials are entered in the **Settings > API** tab and stored encrypted using Flutter Secure Storage (Android Keystore / iOS Keychain). Credentials never leave the device.

### Standalone Daemon

Create `config.json` from the template:

```bash
cp config.example.json config.json
```

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

> **WARNING:** Never commit `config.json` to version control. It is in `.gitignore` by default.

## Security

### Credential Storage

| Platform | Method | Security Level |
|----------|--------|----------------|
| Android | EncryptedSharedPreferences (AES-256-GCM via Keystore) | HIGH |
| iOS | Keychain Services | HIGH |
| Desktop | config.json (restrict with `chmod 600`) | MEDIUM |

### Network Security

- **HTTPS only** - Cleartext (HTTP) traffic blocked via `network_security_config.xml`
- **API signing** - All Kraken requests signed with HMAC-SHA512
- **TLS 1.2+** - Via `rustls-tls` (pure Rust TLS)
- **Domain restriction** - Only `api.kraken.com` whitelisted

### Code Protection (Release Builds)

- **ProGuard/R8** enabled - Code obfuscation and shrinking
- **Flutter obfuscation** - `--obfuscate --split-debug-info`
- **Debug log stripping** - `Log.d/v/i` removed in release

### API Key Permissions (Kraken)

Create an API key with **only** these permissions:
- Query Funds
- Query Open Orders & Trades
- Create & Modify Orders

**Never enable:** Withdraw Funds, Query Ledger Entries

### Android Permissions

```xml
<uses-permission android:name="android.permission.INTERNET"/>
<uses-permission android:name="android.permission.USE_BIOMETRIC"/>
<uses-permission android:name="android.permission.FOREGROUND_SERVICE"/>
<uses-permission android:name="android.permission.FOREGROUND_SERVICE_DATA_SYNC"/>
<uses-permission android:name="android.permission.WAKE_LOCK"/>
<uses-permission android:name="android.permission.RECEIVE_BOOT_COMPLETED"/>
```

See [SECURITY.md](SECURITY.md) for the full security audit and production checklist.

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

### Regenerate Bridge Code

After modifying Rust bridge functions in `rust/src/api/simple.rs`:

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

Built with Rust + Flutter | [Security Report](SECURITY.md) | [Build Instructions](BUILD.md)
