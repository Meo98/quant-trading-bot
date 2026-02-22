import flet as ft
import threading
import logging
import json
import os
import time
import ccxt
from pathlib import Path

import autotrader
from autotrader import MomentumTrader, TelegramNotifier

# Secure local storage file (sits next to the app, excluded by .gitignore)
SECRETS_FILE = Path(__file__).parent / ".app_secrets.json"

GITHUB_DOCS_URL = "https://github.com/Meo98/quant-trading-bot#strategy-settings"

# Default strategy values (mirrors autotrader.py constants)
STRATEGY_DEFAULTS = {
    "max_open_trades": 3,
    "trailing_stop_pct": 10.0,
    "hard_stop_loss_pct": 15.0,
    "min_profit_to_exit": 2.5,
    "pump_min_pct_24h": 5.0,
    "pump_min_volume_eur": 10000,
    "pump_cooldown_min": 30,
    "check_interval": 5,
    "dry_run": False,
}

STRATEGY_HELP = {
    "max_open_trades": "How many coins the bot can hold simultaneously.",
    "trailing_stop_pct": "When a coin drops this % from its peak, it sells.",
    "hard_stop_loss_pct": "Emergency exit if price drops this % from buy price.",
    "min_profit_to_exit": "Trailing stop only activates after this profit %.",
    "pump_min_pct_24h": "Minimum 24h pump % to trigger a buy signal.",
    "pump_min_volume_eur": "Minimum 24h volume in EUR (filters scam coins).",
    "pump_cooldown_min": "Minutes to wait before re-buying same coin.",
    "check_interval": "Seconds between market scan cycles.",
    "dry_run": "Simulation mode: no real trades.",
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
    radar_thread = None
    radar_running = False

    # ── PIN SCREEN ──────────────────────────────────────────────────────────

    pin_icon = ft.Icon(ft.Icons.LOCK_OUTLINE, size=64, color=ft.Colors.GREEN_ACCENT_400)
    has_pin = "pin" in secrets
    pin_title = ft.Text(
        "Enter PIN" if has_pin else "Create a 4-Digit PIN",
        size=22, weight=ft.FontWeight.BOLD
    )
    pin_input = ft.TextField(
        password=True, can_reveal_password=True,
        text_align=ft.TextAlign.CENTER, width=250,
        keyboard_type=ft.KeyboardType.NUMBER, max_length=8, autofocus=True,
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

    pin_submit_btn = ft.Button("Unlock" if has_pin else "Set PIN",
        on_click=on_pin_submit, width=250,
        bgcolor=ft.Colors.GREEN_800, color=ft.Colors.WHITE)

    pin_view = ft.Column(
        [pin_icon, pin_title, pin_input, pin_error, pin_submit_btn,
         ft.Text("💡 Tip: Enable your phone's App Lock for fingerprint/face unlock!",
                  color=ft.Colors.GREY_600, size=11, text_align=ft.TextAlign.CENTER)],
        horizontal_alignment=ft.CrossAxisAlignment.CENTER,
        alignment=ft.MainAxisAlignment.CENTER, spacing=15,
    )

    # ── API SETTINGS ───────────────────────────────────────────────────────

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

    api_settings_view = ft.Column([
        ft.Text("🔑 API Settings", size=22, weight=ft.FontWeight.BOLD),
        ft.Text("Keys are stored locally on this device only.", color=ft.Colors.GREY_400, size=12),
        ft.Divider(), kraken_key_input, kraken_secret_input,
        ft.Divider(), tele_token_input, tele_chat_input, ft.Divider(),
        ft.Button("💾 Save API Keys", on_click=save_api_settings, bgcolor=ft.Colors.BLUE_800, color=ft.Colors.WHITE),
    ], spacing=12, scroll=ft.ScrollMode.ADAPTIVE, expand=True)

    # ── STRATEGY SETTINGS ──────────────────────────────────────────────────

    def _get_strategy() -> dict:
        saved = secrets.get("strategy", {})
        result = dict(STRATEGY_DEFAULTS)
        result.update(saved)
        return result

    def _make_field(label, key, suffix="", is_int=False):
        strat = _get_strategy()
        val = strat.get(key, STRATEGY_DEFAULTS[key])
        tf = ft.TextField(
            label=f"{label} ({suffix})" if suffix else label,
            value=str(int(val) if is_int else val),
            tooltip=STRATEGY_HELP.get(key, ""),
            keyboard_type=ft.KeyboardType.NUMBER, dense=True,
        )
        return tf, key

    field_max_trades, _ = _make_field("Max Open Trades", "max_open_trades", "slots", True)
    field_trailing, _ = _make_field("Trailing Stop", "trailing_stop_pct", "%")
    field_hard_sl, _ = _make_field("Hard Stop Loss", "hard_stop_loss_pct", "%")
    field_min_profit, _ = _make_field("Min Profit to Trail", "min_profit_to_exit", "%")
    field_pump_pct, _ = _make_field("Pump Detection", "pump_min_pct_24h", "% (24h)")
    field_volume, _ = _make_field("Min Volume", "pump_min_volume_eur", "EUR", True)
    field_cooldown, _ = _make_field("Pump Cooldown", "pump_cooldown_min", "min", True)
    field_interval, _ = _make_field("Scan Interval", "check_interval", "sec", True)
    dry_run_switch = ft.Switch(label="🧪 Dry Run (Simulation)", value=_get_strategy().get("dry_run", False))

    ALL_FIELDS = [
        (field_max_trades, "max_open_trades", True), (field_trailing, "trailing_stop_pct", False),
        (field_hard_sl, "hard_stop_loss_pct", False), (field_min_profit, "min_profit_to_exit", False),
        (field_pump_pct, "pump_min_pct_24h", False), (field_volume, "pump_min_volume_eur", True),
        (field_cooldown, "pump_cooldown_min", True), (field_interval, "check_interval", True),
    ]

    def load_strategy_fields():
        strat = _get_strategy()
        for tf, key, is_int in ALL_FIELDS:
            val = strat.get(key, STRATEGY_DEFAULTS[key])
            tf.value = str(int(val) if is_int else val)
        dry_run_switch.value = strat.get("dry_run", False)

    def save_strategy(e):
        nonlocal secrets
        strat = {}
        for tf, key, is_int in ALL_FIELDS:
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
        load_strategy_fields()
        page.snack_bar = ft.SnackBar(ft.Text("🔄 Reset to defaults!"), bgcolor=ft.Colors.ORANGE_800)
        page.snack_bar.open = True
        page.update()

    def open_docs(e):
        page.launch_url(GITHUB_DOCS_URL)

    strategy_view = ft.Column([
        ft.Row([ft.Text("📊 Strategy", size=22, weight=ft.FontWeight.BOLD),
                ft.IconButton(icon=ft.Icons.HELP_OUTLINE, tooltip="Open docs", on_click=open_docs)],
               alignment=ft.MainAxisAlignment.SPACE_BETWEEN),
        ft.Text("Customize how your bot trades.", color=ft.Colors.GREY_400, size=12), ft.Divider(),
        ft.Text("Trade Management", weight=ft.FontWeight.BOLD, color=ft.Colors.GREEN_ACCENT_400, size=14),
        field_max_trades, field_trailing, field_hard_sl, field_min_profit, ft.Divider(),
        ft.Text("Pump Detection", weight=ft.FontWeight.BOLD, color=ft.Colors.ORANGE_ACCENT_400, size=14),
        field_pump_pct, field_volume, field_cooldown, field_interval, ft.Divider(),
        ft.Text("Mode", weight=ft.FontWeight.BOLD, color=ft.Colors.BLUE_ACCENT_400, size=14),
        dry_run_switch, ft.Divider(),
        ft.Row([ft.Button("💾 Save", on_click=save_strategy, bgcolor=ft.Colors.BLUE_800, color=ft.Colors.WHITE),
                ft.Button("🔄 Defaults", on_click=reset_strategy, bgcolor=ft.Colors.GREY_800, color=ft.Colors.WHITE)], spacing=10),
    ], spacing=10, scroll=ft.ScrollMode.ADAPTIVE, expand=True)

    # ── PORTFOLIO SCREEN ───────────────────────────────────────────────────

    portfolio_list = ft.ListView(expand=True, spacing=8)
    portfolio_balance = ft.Text("Loading...", size=28, weight=ft.FontWeight.BOLD, color=ft.Colors.GREEN_ACCENT_400)
    portfolio_status = ft.Text("", color=ft.Colors.GREY_400, size=12)

    def refresh_portfolio(e=None):
        k_key = secrets.get("kraken_key", "")
        k_sec = secrets.get("kraken_secret", "")
        if not k_key or not k_sec:
            portfolio_balance.value = "No API Keys"
            portfolio_status.value = "Go to API Keys tab to connect."
            portfolio_list.controls.clear()
            page.update()
            return

        portfolio_status.value = "⏳ Fetching from Kraken..."
        page.update()

        def _fetch():
            try:
                exchange = ccxt.kraken({
                    'apiKey': k_key, 'secret': k_sec,
                    'enableRateLimit': True,
                })
                balance = exchange.fetch_balance()

                total_eur = 0.0
                holdings = []

                for currency, amount_info in balance.get("total", {}).items():
                    amount = float(amount_info) if amount_info else 0.0
                    if amount < 0.0001:
                        continue

                    if currency in ("EUR", "ZEUR"):
                        total_eur += amount
                        holdings.append(("EUR", amount, amount, 0.0))
                    elif currency in ("USD", "ZUSD", "USDT", "USDC"):
                        holdings.append((currency, amount, amount * 0.92, 0.0))
                        total_eur += amount * 0.92
                    else:
                        # Try to get EUR price
                        pair = f"{currency}/EUR"
                        try:
                            ticker = exchange.fetch_ticker(pair)
                            price = ticker.get("last", 0) or 0
                            value_eur = amount * price
                            pct = ticker.get("percentage", 0) or 0
                            if value_eur > 0.01:
                                total_eur += value_eur
                                holdings.append((currency, amount, value_eur, pct))
                        except Exception:
                            pass

                # Sort by value descending
                holdings.sort(key=lambda x: x[2], reverse=True)

                portfolio_balance.value = f"€{total_eur:,.2f}"
                portfolio_status.value = f"✅ {len(holdings)} assets | Last refresh: now"
                portfolio_list.controls.clear()

                for symbol, amount, value_eur, pct_24h in holdings:
                    pct_color = ft.Colors.GREEN if pct_24h >= 0 else ft.Colors.RED
                    pct_str = f"{pct_24h:+.1f}%" if pct_24h != 0 else ""

                    portfolio_list.controls.append(
                        ft.Container(
                            content=ft.Row([
                                ft.Column([
                                    ft.Text(symbol, weight=ft.FontWeight.BOLD, size=14),
                                    ft.Text(f"{amount:.6g}", color=ft.Colors.GREY_400, size=11),
                                ], spacing=2),
                                ft.Column([
                                    ft.Text(f"€{value_eur:,.2f}", weight=ft.FontWeight.BOLD, size=14,
                                            text_align=ft.TextAlign.RIGHT),
                                    ft.Text(pct_str, color=pct_color, size=11,
                                            text_align=ft.TextAlign.RIGHT) if pct_str else ft.Text(""),
                                ], spacing=2, horizontal_alignment=ft.CrossAxisAlignment.END),
                            ], alignment=ft.MainAxisAlignment.SPACE_BETWEEN),
                            padding=ft.padding.symmetric(horizontal=12, vertical=8),
                            border_radius=8,
                            bgcolor=ft.Colors.with_opacity(0.15, ft.Colors.WHITE),
                        )
                    )
                page.update()

            except Exception as e:
                portfolio_balance.value = "Error"
                portfolio_status.value = f"❌ {str(e)[:80]}"
                page.update()

        threading.Thread(target=_fetch, daemon=True).start()

    portfolio_view = ft.Column([
        ft.Row([ft.Text("💰 Portfolio", size=22, weight=ft.FontWeight.BOLD),
                ft.IconButton(icon=ft.Icons.REFRESH, tooltip="Refresh", on_click=refresh_portfolio)],
               alignment=ft.MainAxisAlignment.SPACE_BETWEEN),
        portfolio_balance, portfolio_status, ft.Divider(), portfolio_list,
    ], expand=True)

    # ── DEX RADAR SCREEN ───────────────────────────────────────────────────

    radar_log = ft.ListView(expand=True, spacing=4, auto_scroll=True)
    radar_status = ft.Text("Idle", color=ft.Colors.GREY_400, size=12)

    def run_radar():
        """Run DexScreener radar loop in background."""
        try:
            import requests
        except ImportError:
            radar_log.controls.append(ft.Text("❌ 'requests' not installed", color=ft.Colors.RED))
            page.update()
            return

        known_pairs = set()
        CHAIN = "solana"
        MIN_LIQ = 10000
        BULLISH = ["gem", "moon", "bullish", "pump", "alpha", "ai", "pepe", "doge"]
        BEARISH = ["rug", "scam", "honeypot", "dump", "fake"]

        radar_status.value = f"🔍 Scanning {CHAIN.upper()} DEXes..."
        page.update()

        # Initial seed
        try:
            resp = requests.get("https://api.dexscreener.com/latest/dex/search?q=solana%20meme",
                                headers={'User-Agent': 'Mozilla/5.0'}, timeout=10)
            for p in resp.json().get("pairs", []):
                if p.get("chainId") == CHAIN:
                    known_pairs.add(p.get("pairAddress", ""))
            radar_log.controls.append(
                ft.Text(f"✅ Indexed {len(known_pairs)} existing pairs", color=ft.Colors.GREEN))
            page.update()
        except Exception as e:
            radar_log.controls.append(ft.Text(f"⚠️ Init error: {e}", color=ft.Colors.YELLOW))
            page.update()

        while radar_running:
            try:
                resp = requests.get("https://api.dexscreener.com/latest/dex/search?q=solana%20meme",
                                    headers={'User-Agent': 'Mozilla/5.0'}, timeout=10)
                for p in resp.json().get("pairs", []):
                    if p.get("chainId") != CHAIN:
                        continue
                    addr = p.get("pairAddress", "")
                    if addr in known_pairs:
                        continue

                    liq = p.get("liquidity", {}).get("usd", 0)
                    vol = p.get("volume", {}).get("h24", 0)
                    if liq < MIN_LIQ:
                        continue

                    known_pairs.add(addr)
                    name = p.get("baseToken", {}).get("name", "?")
                    symbol = p.get("baseToken", {}).get("symbol", "?")
                    price = float(p.get("priceUsd", 0))

                    # Simple sentiment
                    text = f"{name} {symbol}".lower()
                    score = sum(10 for w in BULLISH if w in text) - sum(15 for w in BEARISH if w in text)

                    if score >= 0:
                        emoji = "🔥" if score >= 15 else "💎"
                        color = ft.Colors.GREEN if score >= 15 else ft.Colors.CYAN
                    else:
                        emoji = "🗑️"
                        color = ft.Colors.GREY_600

                    radar_log.controls.append(ft.Text(
                        f"{emoji} {symbol} | ${price:.6f} | Liq: ${liq:,.0f} | Vol: ${vol:,.0f}",
                        color=color, font_family="monospace", size=11))

                    if len(radar_log.controls) > 200:
                        radar_log.controls.pop(0)
                    page.update()

                radar_status.value = f"🔍 Tracking {len(known_pairs)} pairs | Scanning..."
                page.update()

            except Exception as e:
                radar_log.controls.append(ft.Text(f"⚠️ {e}", color=ft.Colors.YELLOW))
                page.update()

            time.sleep(60)

    def toggle_radar(e):
        nonlocal radar_thread, radar_running
        if radar_running:
            radar_running = False
            radar_btn.text = "▶ Start Radar"
            radar_btn.bgcolor = ft.Colors.PURPLE_800
            radar_status.value = "Stopped"
            page.update()
        else:
            radar_running = True
            radar_btn.text = "⏹ Stop Radar"
            radar_btn.bgcolor = ft.Colors.RED_800
            page.update()
            radar_thread = threading.Thread(target=run_radar, daemon=True)
            radar_thread.start()

    radar_btn = ft.Button("▶ Start Radar", on_click=toggle_radar,
                          bgcolor=ft.Colors.PURPLE_800, color=ft.Colors.WHITE, width=200)

    radar_view = ft.Column([
        ft.Text("🛰️ DEX Radar", size=22, weight=ft.FontWeight.BOLD),
        ft.Text("Solana DEX scout — alerts only, no auto-trading.",
                color=ft.Colors.GREY_400, size=12),
        ft.Divider(),
        ft.Row([radar_btn, radar_status], alignment=ft.MainAxisAlignment.SPACE_BETWEEN),
        ft.Divider(),
        ft.Container(content=radar_log, border=ft.Border.all(1, ft.Colors.GREY_800),
                      border_radius=10, padding=10, expand=True,
                      bgcolor=ft.Colors.with_opacity(0.3, ft.Colors.BLACK)),
    ], expand=True)

    # ── DASHBOARD SCREEN ───────────────────────────────────────────────────

    log_view = ft.ListView(expand=True, spacing=4, auto_scroll=True)
    status_text = ft.Text("Status: Idle", color=ft.Colors.GREY_400)

    def run_bot(k_key, k_sec, t_tok, t_chat):
        try:
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
                    log_view.controls.append(ft.Text(msg, color=color, font_family="monospace", size=11))
                    if len(log_view.controls) > 500:
                        log_view.controls.pop(0)
                    page.update()

            flet_handler = FletHandler()
            flet_handler.setFormatter(logging.Formatter("%(asctime)s │ %(message)s", datefmt="%H:%M:%S"))
            autotrader.log.addHandler(flet_handler)

            trader = MomentumTrader(
                api_key=k_key or None, api_secret=k_sec or None,
                tele_token=t_tok or None, tele_chat=t_chat or None,
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
            page.snack_bar = ft.SnackBar(ft.Text("❌ API Keys missing!"), bgcolor=ft.Colors.RED_800)
            page.snack_bar.open = True
            nav_bar.selected_index = 4
            on_nav_change(None)
            return

        bot_running = True
        status_text.value = "Status: ⚡ ACTIVE"
        status_text.color = ft.Colors.GREEN_ACCENT_400
        start_btn.disabled = True
        start_btn.text = "⏳ Running..."
        page.update()

        bot_thread = threading.Thread(
            target=run_bot,
            args=(k_key, k_sec, secrets.get("tele_token", ""), secrets.get("tele_chat", "")),
            daemon=True)
        bot_thread.start()

    start_btn = ft.Button("▶ START BOT", on_click=start_bot_click,
                          bgcolor=ft.Colors.GREEN_800, color=ft.Colors.WHITE, width=200)

    dashboard_view = ft.Column([
        ft.Row([ft.Text("Matrix Quant Trader", size=22, weight=ft.FontWeight.BOLD,
                        color=ft.Colors.GREEN_ACCENT_400)], alignment=ft.MainAxisAlignment.CENTER),
        ft.Divider(),
        ft.Row([start_btn, status_text], alignment=ft.MainAxisAlignment.SPACE_BETWEEN),
        ft.Divider(),
        ft.Container(content=log_view, border=ft.Border.all(1, ft.Colors.GREY_800),
                      border_radius=10, padding=10, expand=True,
                      bgcolor=ft.Colors.with_opacity(0.3, ft.Colors.BLACK)),
    ], expand=True)

    # ── NAVIGATION ─────────────────────────────────────────────────────────

    content_area = ft.Container(expand=True, padding=20)

    def show_tab(index: int):
        if index == 0:
            content_area.content = dashboard_view
        elif index == 1:
            refresh_portfolio()
            content_area.content = portfolio_view
        elif index == 2:
            content_area.content = radar_view
        elif index == 3:
            load_strategy_fields()
            content_area.content = strategy_view
        elif index == 4:
            load_api_settings()
            content_area.content = api_settings_view
        page.update()

    def on_nav_change(e):
        show_tab(nav_bar.selected_index)

    nav_bar = ft.NavigationBar(
        destinations=[
            ft.NavigationBarDestination(icon=ft.Icons.DASHBOARD_OUTLINED, selected_icon=ft.Icons.DASHBOARD, label="Bot"),
            ft.NavigationBarDestination(icon=ft.Icons.ACCOUNT_BALANCE_WALLET_OUTLINED, selected_icon=ft.Icons.ACCOUNT_BALANCE_WALLET, label="Portfolio"),
            ft.NavigationBarDestination(icon=ft.Icons.RADAR_OUTLINED, selected_icon=ft.Icons.RADAR, label="Radar"),
            ft.NavigationBarDestination(icon=ft.Icons.TUNE_OUTLINED, selected_icon=ft.Icons.TUNE, label="Strategy"),
            ft.NavigationBarDestination(icon=ft.Icons.KEY_OUTLINED, selected_icon=ft.Icons.KEY, label="Keys"),
        ],
        on_change=on_nav_change, selected_index=0,
    )

    # ── LAYOUT ─────────────────────────────────────────────────────────────

    def show_main_app():
        page.clean()
        page.vertical_alignment = ft.MainAxisAlignment.START
        page.horizontal_alignment = ft.CrossAxisAlignment.STRETCH
        show_tab(0)
        page.add(content_area)
        page.navigation_bar = nav_bar
        page.update()

    page.add(pin_view)


if __name__ == "__main__":
    ft.run(main, view=ft.AppView.WEB_BROWSER, port=8550)
