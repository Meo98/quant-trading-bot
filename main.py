import flet as ft
import threading
import logging
import os
import autotrader
from autotrader import MomentumTrader

def main(page: ft.Page):
    page.title = "Quant System AutoTrader"
    page.theme_mode = ft.ThemeMode.DARK
    page.vertical_alignment = ft.MainAxisAlignment.CENTER
    page.horizontal_alignment = ft.CrossAxisAlignment.CENTER

    # Read from client_storage
    has_pin = page.client_storage.contains_key("app_pin")
    saved_pin = page.client_storage.get("app_pin") if has_pin else ""
    
    # State flags
    is_authenticated = False
    bot_thread = None
    bot_running = False

    # --- UI Elements ---
    
    # PIN Screen
    pin_icon = ft.Icon(ft.icons.LOCK_OUTLINE, size=64, color=ft.colors.GREEN_ACCENT_400)
    pin_title = ft.Text("Enter PIN to Unlock" if has_pin else "Create a 4-Digit PIN", size=24, weight=ft.FontWeight.BOLD)
    pin_input = ft.TextField(
        password=True, 
        can_reveal_password=True,
        text_align=ft.TextAlign.CENTER, 
        width=250, 
        keyboard_type=ft.KeyboardType.NUMBER
    )
    pin_error = ft.Text("", color=ft.colors.RED)
    
    def on_pin_submit(e):
        nonlocal saved_pin, has_pin, is_authenticated
        entered = pin_input.value
        if len(entered) < 4:
            pin_error.value = "PIN must be at least 4 digits"
            page.update()
            return

        if not has_pin:
            # Create PIN
            page.client_storage.set("app_pin", entered)
            saved_pin = entered
            has_pin = True
            is_authenticated = True
            show_main_layout()
        else:
            # Verify PIN
            if entered == saved_pin:
                is_authenticated = True
                show_main_layout()
            else:
                pin_error.value = "Incorrect PIN"
                pin_input.value = ""
                page.update()

    pin_submit_btn = ft.ElevatedButton("Unlock" if has_pin else "Set PIN", on_click=on_pin_submit, width=250, bgcolor=ft.colors.GREEN_800, color=ft.colors.WHITE)
    
    pin_view = ft.Column(
        [pin_icon, pin_title, pin_input, pin_error, pin_submit_btn],
        horizontal_alignment=ft.CrossAxisAlignment.CENTER,
        alignment=ft.MainAxisAlignment.CENTER
    )

    # --- Main Layout Screens ---
    
    # Setup Screen
    setup_title = ft.Text("API Settings", size=24, weight=ft.FontWeight.BOLD)
    setup_subtitle = ft.Text("Your keys are stored securely on this device.", color=ft.colors.GREY_400)
    
    kraken_key_input = ft.TextField(label="Kraken API Key", password=True, can_reveal_password=True)
    kraken_secret_input = ft.TextField(label="Kraken API Secret", password=True, can_reveal_password=True)
    tele_token_input = ft.TextField(label="Telegram Bot Token (Optional)", password=True, can_reveal_password=True)
    tele_chat_input = ft.TextField(label="Telegram Chat ID (Optional)", password=False)
    
    def load_settings():
        kraken_key_input.value = page.client_storage.get("kraken_key") or ""
        kraken_secret_input.value = page.client_storage.get("kraken_secret") or ""
        tele_token_input.value = page.client_storage.get("tele_token") or ""
        tele_chat_input.value = page.client_storage.get("tele_chat") or ""
    
    def save_settings(e):
        page.client_storage.set("kraken_key", kraken_key_input.value.strip())
        page.client_storage.set("kraken_secret", kraken_secret_input.value.strip())
        page.client_storage.set("tele_token", tele_token_input.value.strip())
        page.client_storage.set("tele_chat", tele_chat_input.value.strip())
        
        page.snack_bar = ft.SnackBar(ft.Text("Settings saved securely!"), bgcolor=ft.colors.GREEN_800)
        page.snack_bar.open = True
        page.update()
        
    save_settings_btn = ft.ElevatedButton("Save Settings", on_click=save_settings, bgcolor=ft.colors.BLUE_800, color=ft.colors.WHITE)
    
    setup_view = ft.ListView(
        controls=[
            setup_title, 
            setup_subtitle,
            ft.Divider(),
            kraken_key_input, 
            kraken_secret_input, 
            tele_token_input, 
            tele_chat_input, 
            save_settings_btn
        ],
        expand=True,
        spacing=20,
        padding=20
    )


    # Dashboard Screen
    dash_header = ft.Text("Matrix Quant Trader", size=24, weight=ft.FontWeight.BOLD, color=ft.colors.GREEN_ACCENT_400)
    status_text = ft.Text("Status: Waiting to start...", color=ft.colors.GREY_400)
    log_view = ft.ListView(expand=True, spacing=10, auto_scroll=True)
    
    def run_bot(k_key, k_sec, t_tok, t_chat):
        try:
            class FletHandler(logging.Handler):
                def emit(self, record):
                    msg = self.format(record)
                    color = ft.colors.WHITE
                    if "BUY" in msg or "PUMP" in msg: color = ft.colors.GREEN
                    if "SELL" in msg or "STOP-LOSS" in msg: color = ft.colors.RED
                    if "WARNING" in msg or "⚠️" in msg: color = ft.colors.YELLOW
                    
                    log_view.controls.append(ft.Text(msg, color=color, font_family="monospace"))
                    page.update()

            flet_handler = FletHandler()
            flet_handler.setFormatter(logging.Formatter("%(asctime)s | %(message)s", datefmt="%H:%M:%S"))
            autotrader.log.addHandler(flet_handler)

            trader = MomentumTrader(api_key=k_key, api_secret=k_sec, tele_token=t_tok, tele_chat=t_chat)
            trader.run()
        except Exception as e:
            log_view.controls.append(ft.Text(f"CRASH: {e}", color=ft.colors.RED))
            page.update()

    def start_bot_click(e):
        nonlocal bot_thread, bot_running
        if bot_running: return
        
        k_key = page.client_storage.get("kraken_key")
        k_sec = page.client_storage.get("kraken_secret")
        
        if not k_key or not k_sec:
            page.snack_bar = ft.SnackBar(ft.Text("❌ ERROR: Kraken Keys missing in Settings!"), bgcolor=ft.colors.RED_800)
            page.snack_bar.open = True
            page.update()
            
            # Switch to settings tab automatically
            page.navigation_bar.selected_index = 1
            nav_change(None)
            return

        t_tok = page.client_storage.get("tele_token")
        t_chat = page.client_storage.get("tele_chat")
        
        bot_running = True
        status_text.value = "Status: ACTIVE (Running in Background)"
        start_btn.disabled = True
        page.update()
        
        bot_thread = threading.Thread(target=run_bot, args=(k_key, k_sec, t_tok, t_chat), daemon=True)
        bot_thread.start()

    start_btn = ft.ElevatedButton("▶ START BOT", on_click=start_bot_click, bgcolor=ft.colors.GREEN_800, color=ft.colors.WHITE)
    
    # Embed the log viewer into a clean container
    log_container = ft.Container(
        content=log_view,
        border=ft.border.all(1, ft.colors.GREY_800),
        border_radius=10,
        padding=10,
        expand=True,
        bgcolor=ft.colors.BLACK
    )

    dash_view = ft.Column([
        ft.Row([dash_header], alignment=ft.MainAxisAlignment.CENTER),
        ft.Divider(),
        ft.Row([start_btn, status_text], alignment=ft.MainAxisAlignment.SPACE_BETWEEN),
        ft.Divider(),
        log_container
    ], expand=True, visible=False, padding=20)

    
    # --- Navigation ---
    setup_view_container = ft.Container(content=setup_view, expand=True, visible=False)
    
    def nav_change(e):
        if page.navigation_bar.selected_index == 0:
            dash_view.visible = True
            setup_view_container.visible = False
        elif page.navigation_bar.selected_index == 1:
            dash_view.visible = False
            setup_view_container.visible = True
            load_settings()
        page.update()

    page.navigation_bar = ft.NavigationBar(
        destinations=[
            ft.NavigationDestination(icon=ft.icons.DASHBOARD_OUTLINED, selected_icon=ft.icons.DASHBOARD, label="Dashboard"),
            ft.NavigationDestination(icon=ft.icons.SETTINGS_OUTLINED, selected_icon=ft.icons.SETTINGS, label="Settings"),
        ],
        on_change=nav_change,
        selected_index=0,
        visible=False
    )
    
    # --- Initial Window Setup ---
    def show_main_layout():
        page.clean()
        page.vertical_alignment = ft.MainAxisAlignment.START
        page.horizontal_alignment = ft.CrossAxisAlignment.START
        page.add(dash_view, setup_view_container)
        page.navigation_bar.visible = True
        nav_change(None)

    # Start by forcing PIN login
    page.add(pin_view)

if __name__ == '__main__':
    ft.app(target=main)
