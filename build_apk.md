# Building the Android APK (Phase 7)

## The Concept
To transform this Kraken AutoTrader and the Web Dashboard into a native Android App, we move away from `Flask` (which runs a local web server) and instead use **Flet**. 

Flet is a framework that allows you to write UI in Python and compile it directly into cross-platform apps (Android, iOS, Windows) using Flutter under the hood.

## Required Steps for the App Build

1. **Install Flet and Buildozer**
   ```bash
   pip install flet buildozer
   ```

2. **Refactor the Dashboard (Future Implementation)**
   The `index.html` and `dashboard.py` would need to be rewritten using Flet's Python components:
   ```python
   import flet as ft
   import json

   def main(page: ft.Page):
       page.title = "Kraken AutoTrader"
       page.theme_mode = ft.ThemeMode.DARK
       
       # Add UI components that interact with the config.json and autotrader.py
       page.add(ft.Text("Command Center", size=30, color=ft.colors.GREEN_ACCENT_400))
       # ...
       
   ft.app(target=main)
   ```

3. **Compiling the APK**
   Once the UI is rewritten in Flet, you can compile the entire Python script + UI into an APK file for your phone:
   ```bash
   flet build apk
   ```

## Background Execution (Battery Life)
The Flet app will run on your phone. To ensure `autotrader.py` continues checking prices every 60 seconds even when the phone screen is off, the app will need an Android WakeLock or a Foreground Service declaration in its AndroidManifest. 

However, since our Phase 7 update introduces **Hard Exchange-Side Stop-Losses**, you can safely close the app completely. If you buy a coin, the stop-loss is already resting on Kraken's servers. You don't need the phone scanning 24/7 to protect your bags!
