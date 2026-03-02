# Matrix Quant ProGuard Rules
# ===========================

# Flutter
-keep class io.flutter.** { *; }
-keep class io.flutter.plugins.** { *; }

# Flutter Rust Bridge
-keep class com.matrixquant.** { *; }

# Flutter Secure Storage
-keep class com.it_nomads.fluttersecurestorage.** { *; }

# Biometric Auth
-keep class androidx.biometric.** { *; }

# Background Service
-keep class id.flutter.flutter_background_service.** { *; }

# Keep native methods
-keepclasseswithmembernames class * {
    native <methods>;
}

# Prevent obfuscation of Rust FFI classes
-keep class * extends com.sun.jna.** { *; }

# Keep serialization
-keepattributes Signature
-keepattributes *Annotation*

# Google Play Core (deferred components)
-dontwarn com.google.android.play.core.splitcompat.SplitCompatApplication
-dontwarn com.google.android.play.core.splitinstall.**
-dontwarn com.google.android.play.core.tasks.**

# Remove logging in release
-assumenosideeffects class android.util.Log {
    public static *** d(...);
    public static *** v(...);
    public static *** i(...);
}
