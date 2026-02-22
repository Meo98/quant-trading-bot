import flet as ft
import threading
import logging
import json
import os
from pathlib import Path

import autotrader
from autotrader import MomentumTrader

# Secure local storage file (sits next to the app, excluded by .gitignore)
SECRETS_FILE = Path(__file__).parent / ".app_secrets.json"

def _load_secrets() -> dict:
    if SECRETS_FILE.exists():
        try:
            with open(SECRETS_FILE) as f:
                return json.load(f)
        except Exception:
            pass
    return {}

def _save_secrets(data: dict):
    with open(SECRETS_FILE, "w") as f:
        json.dump(data, f, indent=2)

# ══════════════════════════════════════════════════════════════════════════════

def main(page: ft.Page):
    page.title = "Matrix Quant Trader"
    page.theme_mode = ft.ThemeMode.DARK
    page.vertical_alignment = ft.MainAxisAlignment.CENTER
    page.horizontal_alignment = ft.CrossAxisAlignment.CENTER
    page.window.width = 420
    page.window.height = 800

    secrets = _load_secrets()
    bot_thread = None
    bot_running = False

    # ── PIN SCREEN ──────────────────────────────────────────────────────────

    pin_icon = ft.Icon(ft.Icons.LOCK_OUTLINE, size=64, color=ft.Colors.GREEN_ACCENT_400)
    has_pin = "pin" in secrets
    pin_title = ft.Text(
        "Enter PIN" if has_pin else "Create a 4-Digit PIN",
        size=22, weight=ft.FontWeight.BOLD
    )
    pin_input = ft.TextField(
        password=True,
        can_reveal_password=True,
        text_align=ft.TextAlign.CENTER,
        width=250,
        keyboard_type=ft.KeyboardType.NUMBER,
        max_length=8,
        autofocus=True,
    )
    pin_error = ft.Text("", color=ft.Colors.RED)

    def on_pin_submit(e):
        nonlocal secrets
        entered = pin_input.value or ""
        if len(entered) < 4:
            pin_error.value = "PIN must be at least 4 digits"
            page.update()
            return

        if "pin" not in secrets:
            # First time: create PIN
            secrets["pin"] = entered
            _save_secrets(secrets)
            show_main_app()
        else:
            # Verify
            if entered == secrets["pin"]:
                show_main_app()
            else:
                pin_error.value = "❌ Wrong PIN"
                pin_input.value = ""
                page.update()

    pin_submit_btn = ft.Button(
        "Unlock" if has_pin else "Set PIN",
        on_click=on_pin_submit,
        width=250,
        bgcolor=ft.Colors.GREEN_800,
        color=ft.Colors.WHITE,
    )

    pin_view = ft.Column(
        [pin_icon, pin_title, pin_input, pin_error, pin_submit_btn],
        horizontal_alignment=ft.CrossAxisAlignment.CENTER,
        alignment=ft.MainAxisAlignment.CENTER,
        spacing=15,
    )

    # ── SETTINGS SCREEN ────────────────────────────────────────────────────

    kraken_key_input = ft.TextField(label="Kraken API Key", password=True, can_reveal_password=True)
    kraken_secret_input = ft.TextField(label="Kraken API Secret", password=True, can_reveal_password=True)
    tele_token_input = ft.TextField(label="Telegram Bot Token (optional)", password=True, can_reveal_password=True)
    tele_chat_input = ft.TextField(label="Telegram Chat ID (optional)")

    def load_settings_into_fields():
        kraken_key_input.value = secrets.get("kraken_key", "")
        kraken_secret_input.value = secrets.get("kraken_secret", "")
        tele_token_input.value = secrets.get("tele_token", "")
        tele_chat_input.value = secrets.get("tele_chat", "")

    def save_settings(e):
        nonlocal secrets
        secrets["kraken_key"] = (kraken_key_input.value or "").strip()
        secrets["kraken_secret"] = (kraken_secret_input.value or "").strip()
        secrets["tele_token"] = (tele_token_input.value or "").strip()
        secrets["tele_chat"] = (tele_chat_input.value or "").strip()
        _save_secrets(secrets)

        page.snack_bar = ft.SnackBar(ft.Text("✅ Settings saved!"), bgcolor=ft.Colors.GREEN_800)
        page.snack_bar.open = True
        page.update()

    settings_view = ft.Column(
        [
            ft.Text("⚙️ API Settings", size=22, weight=ft.FontWeight.BOLD),
            ft.Text("Keys are stored locally on this device only.", color=ft.Colors.GREY_400, size=12),
            ft.Divider(),
            kraken_key_input,
            kraken_secret_input,
            ft.Divider(),
            tele_token_input,
            tele_chat_input,
            ft.Divider(),
            ft.Button("💾 Save Settings", on_click=save_settings, bgcolor=ft.Colors.BLUE_800, color=ft.Colors.WHITE),
        ],
        spacing=12,
        scroll=ft.ScrollMode.ADAPTIVE,
        expand=True,
    )

    # ── DASHBOARD SCREEN ───────────────────────────────────────────────────

    log_view = ft.ListView(expand=True, spacing=4, auto_scroll=True)
    status_text = ft.Text("Status: Idle", color=ft.Colors.GREY_400)

    def run_bot(k_key, k_sec, t_tok, t_chat):
        try:
            class FletHandler(logging.Handler):
                def emit(self, record):
                    msg = self.format(record)
                    color = ft.Colors.WHITE
                    if "BUY" in msg or "PUMP" in msg:
                        color = ft.Colors.GREEN
                    elif "SELL" in msg or "STOP-LOSS" in msg:
                        color = ft.Colors.RED
                    elif "WARNING" in msg or "⚠️" in msg:
                        color = ft.Colors.YELLOW

                    log_view.controls.append(
                        ft.Text(msg, color=color, font_family="monospace", size=11)
                    )
                    # Keep log buffer reasonable
                    if len(log_view.controls) > 500:
                        log_view.controls.pop(0)
                    page.update()

            flet_handler = FletHandler()
            flet_handler.setFormatter(logging.Formatter("%(asctime)s │ %(message)s", datefmt="%H:%M:%S"))
            autotrader.log.addHandler(flet_handler)

            trader = MomentumTrader(
                api_key=k_key or None,
                api_secret=k_sec or None,
                tele_token=t_tok or None,
                tele_chat=t_chat or None,
            )
            trader.run()
        except Exception as e:
            log_view.controls.append(ft.Text(f"💥 CRASH: {e}", color=ft.Colors.RED))
            page.update()

    def start_bot_click(e):
        nonlocal bot_thread, bot_running
        if bot_running:
            return

        k_key = secrets.get("kraken_key", "")
        k_sec = secrets.get("kraken_secret", "")

        if not k_key or not k_sec:
            page.snack_bar = ft.SnackBar(
                ft.Text("❌ Kraken API Keys missing! Go to Settings first."),
                bgcolor=ft.Colors.RED_800,
            )
            page.snack_bar.open = True
            # Switch to settings tab
            nav_bar.selected_index = 1
            on_nav_change(None)
            return

        t_tok = secrets.get("tele_token", "")
        t_chat = secrets.get("tele_chat", "")

        bot_running = True
        status_text.value = "Status: ⚡ ACTIVE"
        status_text.color = ft.Colors.GREEN_ACCENT_400
        start_btn.disabled = True
        start_btn.text = "⏳ Running..."
        page.update()

        bot_thread = threading.Thread(
            target=run_bot, args=(k_key, k_sec, t_tok, t_chat), daemon=True
        )
        bot_thread.start()

    start_btn = ft.Button(
        "▶ START BOT",
        on_click=start_bot_click,
        bgcolor=ft.Colors.GREEN_800,
        color=ft.Colors.WHITE,
        width=200,
    )

    dashboard_view = ft.Column(
        [
            ft.Row(
                [ft.Text("Matrix Quant Trader", size=22, weight=ft.FontWeight.BOLD, color=ft.Colors.GREEN_ACCENT_400)],
                alignment=ft.MainAxisAlignment.CENTER,
            ),
            ft.Divider(),
            ft.Row([start_btn, status_text], alignment=ft.MainAxisAlignment.SPACE_BETWEEN),
            ft.Divider(),
            ft.Container(
                content=log_view,
                border=ft.Border.all(1, ft.Colors.GREY_800),
                border_radius=10,
                padding=10,
                expand=True,
                bgcolor=ft.Colors.with_opacity(0.3, ft.Colors.BLACK),
            ),
        ],
        expand=True,
    )

    # ── NAVIGATION ─────────────────────────────────────────────────────────

    content_area = ft.Container(expand=True, padding=20)

    def show_tab(index: int):
        if index == 0:
            content_area.content = dashboard_view
        elif index == 1:
            load_settings_into_fields()
            content_area.content = settings_view
        page.update()

    def on_nav_change(e):
        show_tab(nav_bar.selected_index)

    nav_bar = ft.NavigationBar(
        destinations=[
            ft.NavigationBarDestination(icon=ft.Icons.DASHBOARD_OUTLINED, selected_icon=ft.Icons.DASHBOARD, label="Dashboard"),
            ft.NavigationBarDestination(icon=ft.Icons.SETTINGS_OUTLINED, selected_icon=ft.Icons.SETTINGS, label="Settings"),
        ],
        on_change=on_nav_change,
        selected_index=0,
    )

    # ── LAYOUT SWITCH ──────────────────────────────────────────────────────

    def show_main_app():
        page.clean()
        page.vertical_alignment = ft.MainAxisAlignment.START
        page.horizontal_alignment = ft.CrossAxisAlignment.STRETCH
        show_tab(0)
        page.add(content_area)
        page.navigation_bar = nav_bar
        page.update()

    # Start with PIN screen
    page.add(pin_view)


if __name__ == "__main__":
    ft.run(main, view=ft.AppView.WEB_BROWSER, port=8550)
