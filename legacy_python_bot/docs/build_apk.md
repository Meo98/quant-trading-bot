# 📱 Building the Android APK

## How It Works
The Matrix Quant Trader app is built with **Flet** (Python → Flutter → Android APK).  
The GitHub Actions workflow automatically builds the APK for you on every push.

## Download the APK
1. Go to your GitHub repo → **Actions** tab
2. Click the latest successful **"Build Android APK"** run
3. Download the **APK artifact** at the bottom
4. Transfer to your Android phone and install

## First Launch
1. **Set a PIN** (4+ digits) — required on every app start
2. Go to **API Keys** tab → enter your Kraken API Key & Secret
3. Go to **Strategy** tab → adjust settings or keep defaults
4. Go to **Dashboard** → tap **▶ START BOT**

## 🔒 Security: Fingerprint / Face Unlock

The app uses a PIN code for authentication. For additional biometric security  
(fingerprint or face unlock), use your phone's built-in **App Lock** feature:

- **Samsung**: Settings → Biometrics → Secure Folder, or use "Lock apps" in Good Lock
- **Xiaomi/POCO**: Settings → Apps → App lock → Enable for "Matrix Quant Trader"
- **OnePlus**: Settings → Privacy → App lock
- **Pixel**: Use a third-party app like "Norton App Lock" or "AppLock"

This gives you **fingerprint/face unlock on top of the PIN**, providing two layers of security.

## Building Locally (Advanced)
```bash
pip install flet
flet build apk
```
The APK will be in `build/apk/`.
