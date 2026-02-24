# Matrix Quant System ⚡

Welcome to the **Matrix Quant System**. This repository contains the completely overhauled trading engine designed for maximum efficiency, zero battery drain on mobile, and 24/7 standalone background execution.

## 🏗 Architecture
The system has evolved from its Python roots into a high-performance native stack:
- **Rust Engine (`rust/`)**: The core logic has been rewritten in Rust for absolute memory safety and blistering speed. It handles all Kraken API interactions, pump detection, volatility-based dynamic stops, and circuit breakers.
- **Flutter App (`lib/`)**: A cross-platform mobile frontend with a stunning Glassmorphism UI. It connects to the Rust engine seamlessly via `flutter_rust_bridge`.
- **Background Daemon**: The app utilizes an Android Foreground Service to spawn a detached isolate. When you close the app, the Rust engine continues monitoring the markets 24/7 and keeps you updated via a persistent Android notification.
- **Biometric Security**: Access to the app is secured via FaceID/Fingerprint authentication so that only you can control the quant engine.

## 🚀 Getting Started

### 1. Build the Mobile App (Android/iOS)
You can clone this repository to your machine and build the app using Flutter:
```bash
# Get the dependencies
flutter pub get

# Build the Android APK
flutter build apk --release
```

### 2. Standalone Linux Daemon (Optional)
If you want to run the Rust engine purely on a Linux server/workstation without the Flutter UI, a CLI daemon entrypoint is available:
```bash
cd rust
cargo build --release
```
You can map this binary to a `systemd` service for fully automated background execution.

## 🏺 Legacy Python System
The original `autotrader.py` and Flask dashboard have been deprecated in favor of this new highly-efficient native architecture. All original Python scripts are preserved in the `legacy_python_bot/` directory for reference and backtesting purposes.

---
*Built for efficiency. Engineered for momentum.*
