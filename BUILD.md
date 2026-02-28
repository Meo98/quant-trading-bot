# Build Instructions - Matrix Quant

## Prerequisites

### Required Software

| Tool | Version | Installation |
|------|---------|--------------|
| Flutter | 3.10+ | https://flutter.dev/docs/get-started/install |
| Rust | 1.70+ | https://rustup.rs |
| Android SDK | 33+ | Via Android Studio |
| JDK | 17 | Via Android Studio |

### Verify Installation

```bash
flutter doctor
rustc --version
cargo --version
```

---

## Android APK Build

### 1. Install Dependencies

```bash
cd /path/to/trading-bot

# Flutter dependencies
flutter pub get

# Generate Rust bridge bindings
flutter_rust_bridge_codegen generate
```

### 2. Build Debug APK (Testing)

```bash
flutter build apk --debug
```

Output: `build/app/outputs/flutter-apk/app-debug.apk`

### 3. Build Release APK (Production)

```bash
# With obfuscation (recommended)
flutter build apk --release --obfuscate --split-debug-info=build/debug-info

# Or simple release
flutter build apk --release
```

Output: `build/app/outputs/flutter-apk/app-release.apk`

### 4. Build Split APKs (Smaller Size)

```bash
flutter build apk --split-per-abi --release
```

Outputs:
- `app-arm64-v8a-release.apk` (ARM64, most devices)
- `app-armeabi-v7a-release.apk` (ARM32, older devices)
- `app-x86_64-release.apk` (Intel/AMD)

---

## Linux Desktop Build

### 1. Enable Desktop Support

```bash
flutter config --enable-linux-desktop
```

### 2. Install Linux Dependencies

```bash
# Ubuntu/Debian
sudo apt install clang cmake ninja-build pkg-config \
  libgtk-3-dev libblkid-dev liblzma-dev libsecret-1-dev

# Arch Linux
sudo pacman -S clang cmake ninja gtk3 libsecret
```

### 3. Build

```bash
flutter build linux --release
```

Output: `build/linux/x64/release/bundle/`

### 4. Create AppImage (Optional)

```bash
# Install appimagetool
wget https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage
chmod +x appimagetool-x86_64.AppImage

# Create AppDir structure
mkdir -p AppDir/usr/bin
cp -r build/linux/x64/release/bundle/* AppDir/usr/bin/
# Add .desktop file and icon...

# Build AppImage
./appimagetool-x86_64.AppImage AppDir MatrixQuant.AppImage
```

---

## Windows Desktop Build

### 1. Enable Windows Support

```bash
flutter config --enable-windows-desktop
```

### 2. Build

```bash
flutter build windows --release
```

Output: `build/windows/x64/runner/Release/`

### 3. Create Installer (Optional)

Use [Inno Setup](https://jrsoftware.org/isinfo.php) or [MSIX](https://docs.microsoft.com/en-us/windows/msix/).

---

## macOS Desktop Build

### 1. Enable macOS Support

```bash
flutter config --enable-macos-desktop
```

### 2. Build

```bash
flutter build macos --release
```

Output: `build/macos/Build/Products/Release/Matrix Quant.app`

---

## Standalone Rust Daemon (Server/Headless)

For running on a server without GUI:

```bash
cd rust
cargo build --release
```

Output: `rust/target/release/matrix-quant-core`

### Run as Systemd Service

```bash
# Create service file
sudo nano /etc/systemd/system/matrix-quant.service
```

```ini
[Unit]
Description=Matrix Quant Trading Engine
After=network.target

[Service]
Type=simple
User=your-user
WorkingDirectory=/path/to/trading-bot
ExecStart=/path/to/trading-bot/rust/target/release/matrix-quant-core
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable matrix-quant
sudo systemctl start matrix-quant
sudo systemctl status matrix-quant
```

---

## Release Signing (Android)

### 1. Create Keystore

```bash
keytool -genkey -v -keystore matrix-quant-release.jks \
  -keyalg RSA -keysize 2048 -validity 10000 \
  -alias matrix-quant
```

### 2. Create key.properties

Create `android/key.properties`:

```properties
storePassword=YOUR_STORE_PASSWORD
keyPassword=YOUR_KEY_PASSWORD
keyAlias=matrix-quant
storeFile=/path/to/matrix-quant-release.jks
```

### 3. Update build.gradle.kts

Edit `android/app/build.gradle.kts`:

```kotlin
import java.util.Properties
import java.io.FileInputStream

val keystorePropertiesFile = rootProject.file("key.properties")
val keystoreProperties = Properties()
if (keystorePropertiesFile.exists()) {
    keystoreProperties.load(FileInputStream(keystorePropertiesFile))
}

android {
    signingConfigs {
        create("release") {
            keyAlias = keystoreProperties["keyAlias"] as String
            keyPassword = keystoreProperties["keyPassword"] as String
            storeFile = file(keystoreProperties["storeFile"] as String)
            storePassword = keystoreProperties["storePassword"] as String
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            signingConfig = signingConfigs.getByName("release")
        }
    }
}
```

---

## Troubleshooting

### Flutter Rust Bridge Issues

```bash
# Regenerate bindings
flutter_rust_bridge_codegen generate --watch

# Clean and rebuild
flutter clean
cargo clean -p matrix-quant-core
flutter pub get
flutter build apk
```

### Gradle Build Failures

```bash
cd android
./gradlew clean
cd ..
flutter build apk
```

### Rust Compilation Errors

```bash
cd rust
cargo check
cargo build 2>&1 | head -50
```

---

## Version Bumping

Edit `pubspec.yaml`:

```yaml
version: 1.0.1+2  # versionName+versionCode
```

---

## CI/CD (GitHub Actions)

See `.github/workflows/build-apk.yml` for automated builds.

---

*Happy Trading!*
