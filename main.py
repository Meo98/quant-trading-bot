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

GITHUB_DOCS_URL = "https://github.com/Meo98/quant-trading-bot#strategy-settings"

# Default strategy values (mirrors autotrader.py constants)
STRATEGY_DEFAULTS = {
    "max_open_trades": 3,
    "trailing_stop_pct": 10.0,      # displayed as % (stored as 0.10 internally)
    "hard_stop_loss_pct": 15.0,     # displayed as % (stored as -0.15 internally)
    "min_profit_to_exit": 2.5,      # displayed as %
    "pump_min_pct_24h": 5.0,
    "pump_min_volume_eur": 10000,
    "pump_cooldown_min": 30,
    "check_interval": 5,
    "dry_run": False,
}

# Tooltips explaining each setting
STRATEGY_HELP = {
    "max_open_trades": "How many coins the bot can hold simultaneously. More slots = more risk but more opportunities.",
    "trailing_stop_pct": "When a coin drops this % from its highest price after buying, it sells. Lower = safer but exits faster.",
    "hard_stop_loss_pct": "Emergency exit: if a coin drops this % from your buy price, sell immediately. Your safety net.",
    "min_profit_to_exit": "The trailing stop only activates after this minimum profit is reached. Prevents selling too early.",
    "pump_min_pct_24h": "Minimum 24h price increase to consider a coin as 'pumping'. Lower = more trades, higher = pickier.",
    "pump_min_volume_eur": "Minimum 24h trading volume in EUR. Filters out low-liquidity scam coins.",
    "pump_cooldown_min": "After selling a coin, wait this many minutes before buying it again.",
    "check_interval": "Seconds between each market scan cycle. Lower = faster reaction but more API calls.",
    "dry_run": "Simulation mode: no real trades, no real money. Perfect for testing your settings!",
}


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
            secrets["pin"] = entered
            _save_secrets(secrets)
            show_main_app()
        else:
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
        [pin_icon, pin_title, pin_input, pin_error, pin_submit_btn,
         ft.Text(
             "💡 Tip: Enable your phone's App Lock for fingerprint/face unlock!",
             color=ft.Colors.GREY_600, size=11, text_align=ft.TextAlign.CENTER,
         )],
        horizontal_alignment=ft.CrossAxisAlignment.CENTER,
        alignment=ft.MainAxisAlignment.CENTER,
        spacing=15,
    )

    # ── API SETTINGS SCREEN ────────────────────────────────────────────────

    kraken_key_input = ft.TextField(label="Kraken API Key", password=True, can_reveal_password=True)
    kraken_secret_input = ft.TextField(label="Kraken API Secret", password=True, can_reveal_password=True)
    tele_token_input = ft.TextField(label="Telegram Bot Token (optional)", password=True, can_reveal_password=True)
    tele_chat_input = ft.TextField(label="Telegram Chat ID (optional)")

    def load_api_settings():
        kraken_key_input.value = secrets.get("kraken_key", "")
        kraken_secret_input.value = secrets.get("kraken_secret", "")
        tele_token_input.value = secrets.get("tele_token", "")
        tele_chat_input.value = secrets.get("tele_chat", "")

    def save_api_settings(e):
        nonlocal secrets
        secrets["kraken_key"] = (kraken_key_input.value or "").strip()
        secrets["kraken_secret"] = (kraken_secret_input.value or "").strip()
        secrets["tele_token"] = (tele_token_input.value or "").strip()
        secrets["tele_chat"] = (tele_chat_input.value or "").strip()
        _save_secrets(secrets)
        page.snack_bar = ft.SnackBar(ft.Text("✅ API Settings saved!"), bgcolor=ft.Colors.GREEN_800)
        page.snack_bar.open = True
        page.update()

    api_settings_view = ft.Column(
        [
            ft.Text("🔑 API Settings", size=22, weight=ft.FontWeight.BOLD),
            ft.Text("Keys are stored locally on this device only.", color=ft.Colors.GREY_400, size=12),
            ft.Divider(),
            kraken_key_input,
            kraken_secret_input,
            ft.Divider(),
            tele_token_input,
            tele_chat_input,
            ft.Divider(),
            ft.Button("💾 Save API Keys", on_click=save_api_settings, bgcolor=ft.Colors.BLUE_800, color=ft.Colors.WHITE),
        ],
        spacing=12,
        scroll=ft.ScrollMode.ADAPTIVE,
        expand=True,
    )

    # ── STRATEGY SETTINGS SCREEN ───────────────────────────────────────────

    def _get_strategy() -> dict:
        """Get saved strategy or defaults."""
        saved = secrets.get("strategy", {})
        result = dict(STRATEGY_DEFAULTS)
        result.update(saved)
        return result

    def _make_setting_row(label: str, key: str, suffix: str = "", is_int: bool = False):
        """Create a labeled TextField for a strategy setting."""
        strat = _get_strategy()
        val = strat.get(key, STRATEGY_DEFAULTS[key])
        tf = ft.TextField(
            label=f"{label} ({suffix})" if suffix else label,
            value=str(int(val) if is_int else val),
            tooltip=STRATEGY_HELP.get(key, ""),
            keyboard_type=ft.KeyboardType.NUMBER,
            dense=True,
        )
        return tf, key

    # Build strategy fields
    field_max_trades, _ = _make_setting_row("Max Open Trades", "max_open_trades", "slots", is_int=True)
    field_trailing, _ = _make_setting_row("Trailing Stop", "trailing_stop_pct", "%")
    field_hard_sl, _ = _make_setting_row("Hard Stop Loss", "hard_stop_loss_pct", "%")
    field_min_profit, _ = _make_setting_row("Min Profit to Trail", "min_profit_to_exit", "%")
    field_pump_pct, _ = _make_setting_row("Pump Detection", "pump_min_pct_24h", "% (24h)")
    field_volume, _ = _make_setting_row("Min Volume", "pump_min_volume_eur", "EUR", is_int=True)
    field_cooldown, _ = _make_setting_row("Pump Cooldown", "pump_cooldown_min", "min", is_int=True)
    field_interval, _ = _make_setting_row("Scan Interval", "check_interval", "sec", is_int=True)
    dry_run_switch = ft.Switch(label="🧪 Dry Run (Simulation Mode)", value=_get_strategy().get("dry_run", False))

    ALL_STRATEGY_FIELDS = [
        (field_max_trades, "max_open_trades", True),
        (field_trailing, "trailing_stop_pct", False),
        (field_hard_sl, "hard_stop_loss_pct", False),
        (field_min_profit, "min_profit_to_exit", False),
        (field_pump_pct, "pump_min_pct_24h", False),
        (field_volume, "pump_min_volume_eur", True),
        (field_cooldown, "pump_cooldown_min", True),
        (field_interval, "check_interval", True),
    ]

    def load_strategy_into_fields():
        strat = _get_strategy()
        for tf, key, is_int in ALL_STRATEGY_FIELDS:
            val = strat.get(key, STRATEGY_DEFAULTS[key])
            tf.value = str(int(val) if is_int else val)
        dry_run_switch.value = strat.get("dry_run", False)

    def save_strategy(e):
        nonlocal secrets
        strat = {}
        for tf, key, is_int in ALL_STRATEGY_FIELDS:
            try:
                strat[key] = int(tf.value) if is_int else float(tf.value)
            except (ValueError, TypeError):
                strat[key] = STRATEGY_DEFAULTS[key]
        strat["dry_run"] = dry_run_switch.value
        secrets["strategy"] = strat
        _save_secrets(secrets)
        page.snack_bar = ft.SnackBar(ft.Text("✅ Strategy saved!"), bgcolor=ft.Colors.GREEN_800)
        page.snack_bar.open = True
        page.update()

    def reset_strategy(e):
        nonlocal secrets
        secrets.pop("strategy", None)
        _save_secrets(secrets)
        load_strategy_into_fields()
        page.snack_bar = ft.SnackBar(ft.Text("🔄 Strategy reset to defaults!"), bgcolor=ft.Colors.ORANGE_800)
        page.snack_bar.open = True
        page.update()

    def open_docs(e):
        page.launch_url(GITHUB_DOCS_URL)

    strategy_view = ft.Column(
        [
            ft.Row([
                ft.Text("📊 Strategy Settings", size=22, weight=ft.FontWeight.BOLD),
                ft.IconButton(icon=ft.Icons.HELP_OUTLINE, tooltip="Open docs on GitHub", on_click=open_docs),
            ], alignment=ft.MainAxisAlignment.SPACE_BETWEEN),
            ft.Text("Customize how your bot trades. Tap ❓ for docs.", color=ft.Colors.GREY_400, size=12),
            ft.Divider(),

            # Trade Management
            ft.Text("Trade Management", weight=ft.FontWeight.BOLD, color=ft.Colors.GREEN_ACCENT_400, size=14),
            field_max_trades,
            field_trailing,
            field_hard_sl,
            field_min_profit,
            ft.Divider(),

            # Pump Detection
            ft.Text("Pump Detection", weight=ft.FontWeight.BOLD, color=ft.Colors.ORANGE_ACCENT_400, size=14),
            field_pump_pct,
            field_volume,
            field_cooldown,
            field_interval,
            ft.Divider(),

            # Mode
            ft.Text("Mode", weight=ft.FontWeight.BOLD, color=ft.Colors.BLUE_ACCENT_400, size=14),
            dry_run_switch,
            ft.Divider(),

            # Action Buttons
            ft.Row([
                ft.Button("💾 Save Strategy", on_click=save_strategy, bgcolor=ft.Colors.BLUE_800, color=ft.Colors.WHITE),
                ft.Button("🔄 Reset Defaults", on_click=reset_strategy, bgcolor=ft.Colors.GREY_800, color=ft.Colors.WHITE),
            ], spacing=10),
        ],
        spacing=10,
        scroll=ft.ScrollMode.ADAPTIVE,
        expand=True,
    )

    # ── DASHBOARD SCREEN ───────────────────────────────────────────────────

    log_view = ft.ListView(expand=True, spacing=4, auto_scroll=True)
    status_text = ft.Text("Status: Idle", color=ft.Colors.GREY_400)

    def run_bot(k_key, k_sec, t_tok, t_chat):
        try:
            # Apply strategy settings to autotrader module globals
            strat = _get_strategy()
            autotrader.MAX_OPEN_TRADES = int(strat.get("max_open_trades", 3))
            autotrader.TRAILING_STOP_PCT = float(strat.get("trailing_stop_pct", 10.0)) / 100.0
            autotrader.HARD_STOP_LOSS_PCT = -abs(float(strat.get("hard_stop_loss_pct", 15.0))) / 100.0
            autotrader.MIN_PROFIT_TO_EXIT = float(strat.get("min_profit_to_exit", 2.5)) / 100.0
            autotrader.PUMP_MIN_PCT_24H = float(strat.get("pump_min_pct_24h", 5.0))
            autotrader.PUMP_MIN_VOLUME_EUR = int(strat.get("pump_min_volume_eur", 10000))
            autotrader.PUMP_COOLDOWN_MIN = int(strat.get("pump_cooldown_min", 30))
            autotrader.CHECK_INTERVAL = int(strat.get("check_interval", 5))
            autotrader.DRY_RUN = bool(strat.get("dry_run", False))

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
            load_strategy_into_fields()
            content_area.content = strategy_view
        elif index == 2:
            load_api_settings()
            content_area.content = api_settings_view
        page.update()

    def on_nav_change(e):
        show_tab(nav_bar.selected_index)

    nav_bar = ft.NavigationBar(
        destinations=[
            ft.NavigationBarDestination(icon=ft.Icons.DASHBOARD_OUTLINED, selected_icon=ft.Icons.DASHBOARD, label="Dashboard"),
            ft.NavigationBarDestination(icon=ft.Icons.TUNE_OUTLINED, selected_icon=ft.Icons.TUNE, label="Strategy"),
            ft.NavigationBarDestination(icon=ft.Icons.KEY_OUTLINED, selected_icon=ft.Icons.KEY, label="API Keys"),
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
