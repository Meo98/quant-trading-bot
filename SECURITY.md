# Security Audit Report - Matrix Quant

## Executive Summary

This document provides a security analysis of the Matrix Quant trading application.
The app handles sensitive financial data and API credentials, requiring high security standards.

---

## Security Assessment

### 1. Credential Storage

| Platform | Method | Security Level |
|----------|--------|----------------|
| Android | EncryptedSharedPreferences (AES-256-GCM) | HIGH |
| iOS | Keychain Services | HIGH |
| Linux Desktop | config.json (plaintext) | MEDIUM* |

*Desktop users should ensure file permissions are restricted (`chmod 600 config.json`)

**Implementation:** `flutter_secure_storage` package
- Uses Android Keystore for key management
- Hardware-backed security on supported devices
- Encrypted at rest

### 2. Network Security

| Aspect | Status | Notes |
|--------|--------|-------|
| HTTPS Only | YES | Enforced via `network_security_config.xml` |
| Cleartext Blocked | YES | `cleartextTrafficPermitted="false"` in manifest |
| Domain Whitelist | YES | Only `api.kraken.com` (with subdomains) |
| Certificate Pinning | NO | Recommended for production |
| API Signing | YES | HMAC-SHA512 per Kraken spec |
| TLS Implementation | rustls | Pure Rust, no OpenSSL dependency |

**Android Network Security Config (`res/xml/network_security_config.xml`):**
```xml
<base-config cleartextTrafficPermitted="false">
    <trust-anchors><certificates src="system" /></trust-anchors>
</base-config>
<domain-config cleartextTrafficPermitted="false">
    <domain includeSubdomains="true">api.kraken.com</domain>
</domain-config>
```

### 3. Authentication

| Feature | Status |
|---------|--------|
| Biometric Auth | YES (FaceID/Fingerprint) |
| PIN Fallback | YES |
| Session Timeout | NO* |

*Recommendation: Add session timeout after X minutes of inactivity

### 4. Android Permissions

```xml
<uses-permission android:name="android.permission.INTERNET"/>
<uses-permission android:name="android.permission.USE_BIOMETRIC"/>
<uses-permission android:name="android.permission.FOREGROUND_SERVICE"/>
<uses-permission android:name="android.permission.WAKE_LOCK"/>
<uses-permission android:name="android.permission.RECEIVE_BOOT_COMPLETED"/>
```

**Assessment:** MINIMAL - Only required permissions are requested.

### 5. Code Obfuscation

| Setting | Status | Notes |
|---------|--------|-------|
| ProGuard/R8 | ENABLED | `isMinifyEnabled = true`, `isShrinkResources = true` |
| Custom ProGuard Rules | ENABLED | `proguard-rules.pro` with keep rules for Flutter, Rust FFI, Secure Storage |
| Debug Log Stripping | ENABLED | `Log.d/v/i` removed in release via ProGuard |
| Flutter obfuscation | AVAILABLE | Use `--obfuscate --split-debug-info` flag at build time |

---

## Identified Issues

### HIGH Priority

1. **Debug Signing for Release Builds**
   - Location: `android/app/build.gradle.kts:36`
   - Issue: Release builds use debug signing config
   - Risk: App can be modified and resigned
   - Fix: Create proper release keystore (see BUILD.md for instructions)

2. ~~**No ProGuard Rules**~~ **FIXED**
   - ProGuard/R8 enabled with `isMinifyEnabled = true`, `isShrinkResources = true`
   - Custom `proguard-rules.pro` with keep rules for Flutter, Rust FFI, Secure Storage, Biometric
   - Debug logs (`Log.d/v/i`) stripped in release builds

### MEDIUM Priority

3. **No Certificate Pinning**
   - Issue: Man-in-the-middle attacks possible
   - Risk: API credentials could be intercepted
   - Fix: Implement certificate pinning for Kraken API

4. **Desktop Config in Plaintext**
   - Issue: `config.json` contains API keys in plaintext
   - Risk: Anyone with file access can read credentials
   - Fix: Use system keyring on desktop

### LOW Priority

5. **No Rate Limiting on Auth**
   - Issue: No limit on biometric auth attempts
   - Risk: Brute force possible (mitigated by OS)

6. **Logging in Release**
   - Issue: Some log statements may leak info
   - Fix: Conditional logging for release builds

---

## Recommendations for Production Release

### 1. Create Release Keystore

```bash
keytool -genkey -v -keystore matrix-quant-release.jks \
  -keyalg RSA -keysize 2048 -validity 10000 \
  -alias matrix-quant
```

### 2. Build with Obfuscation

```bash
flutter build apk --release --obfuscate --split-debug-info=build/debug-info
```

> **Note:** ProGuard/R8 and network security config are already configured. See `android/app/build.gradle.kts`, `android/app/proguard-rules.pro`, and `android/app/src/main/res/xml/network_security_config.xml`.

### 3. Add Certificate Pinning (Optional)

To add certificate pinning for Kraken API, update `network_security_config.xml`:

```xml
<domain-config cleartextTrafficPermitted="false">
    <domain includeSubdomains="true">api.kraken.com</domain>
    <pin-set>
        <!-- Add Kraken certificate SHA-256 pins -->
        <pin digest="SHA-256">BASE64_ENCODED_PIN_HERE</pin>
    </pin-set>
</domain-config>
```

---

## API Key Security Best Practices

### For Users

1. **Create Limited API Keys**
   - Only enable: Query Funds, Query Orders, Create Orders
   - NEVER enable: Withdraw Funds

2. **IP Whitelisting**
   - Restrict API key to your IP addresses if possible

3. **Regular Key Rotation**
   - Change API keys periodically (every 90 days)

4. **Monitor API Usage**
   - Check Kraken dashboard for unusual activity

### For Developers

1. Never log API keys or secrets
2. Clear sensitive data from memory when done
3. Use secure random for nonces
4. Validate all API responses

---

## Compliance Checklist

- [x] ProGuard/R8 enabled (`build.gradle.kts`)
- [ ] Release keystore created (currently uses debug signing)
- [x] Flutter obfuscation available (`--obfuscate` flag)
- [x] Debug logs stripped in release (ProGuard rules)
- [ ] Certificate pinning implemented
- [x] Network security config added (`network_security_config.xml`)
- [x] Cleartext traffic blocked
- [x] API credentials excluded from git (`.gitignore`)
- [x] Git history cleaned of secrets (`git filter-branch`)
- [ ] Privacy policy created
- [ ] Terms of service created

---

## Conclusion

The Matrix Quant app has a **strong security posture** with:
- Encrypted credential storage (AES-256-GCM via Android Keystore)
- Biometric authentication (FaceID / Fingerprint)
- Minimal Android permissions (only what's needed)
- HTTPS-only communication (cleartext blocked)
- Network security config with domain whitelisting
- ProGuard/R8 code obfuscation and shrinking
- Debug log stripping in release builds
- Sensitive files excluded from version control

**Before publishing**, address the remaining issues:
1. Create a proper release signing keystore (HIGH)
2. Consider certificate pinning for Kraken API (MEDIUM)
3. Add session timeout on inactivity (LOW)

Overall Security Rating: **A-** (Strong, with minor improvements possible)

---

*Report generated: 2026-03-01*
*Last updated: 2026-03-01*
*Auditor: Claude Code Security Analysis*
