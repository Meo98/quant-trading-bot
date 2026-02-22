import flet as ft
import threading
import time
import os
import json
import logging
from pathlib import Path

# Wir importieren den Bot manuell
import autotrader
from autotrader import MomentumTrader, DRY_RUN, log_dir

def main(page: ft.Page):
    page.title = "Quant System AutoTrader"
    page.theme_mode = ft.ThemeMode.DARK
    page.vertical_alignment = ft.MainAxisAlignment.START
    page.scroll = ft.ScrollMode.ADAPTIVE

    header = ft.Text("Matrix Quant Trader", size=30, weight=ft.FontWeight.BOLD, color=ft.colors.GREEN_ACCENT_400)
    
    status_text = ft.Text("Zustand: Warte auf Start...", color=ft.colors.GREY_400)
    
    log_view = ft.ListView(expand=True, spacing=10, auto_scroll=True)
    
    bot_thread = None
    bot_running = False

    def run_bot():
        try:
            # Richte Logging für GUI ein (schreibt in log_view)
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

            trader = MomentumTrader()
            trader.run()
        except Exception as e:
            log_view.controls.append(ft.Text(f"CRASH: {e}", color=ft.colors.RED))
            page.update()

    def start_bot_click(e):
        nonlocal bot_thread, bot_running
        if bot_running:
            return
        
        # Check config exists
        config_path = Path("config.json")
        if not config_path.exists():
            log_view.controls.append(ft.Text("❌ FEHLER: config.json fehlt!", color=ft.colors.RED))
            page.update()
            return
            
        bot_running = True
        status_text.value = "Zustand: AKTIV (Hintergrund)"
        start_btn.disabled = True
        page.update()
        
        bot_thread = threading.Thread(target=run_bot, daemon=True)
        bot_thread.start()

    start_btn = ft.ElevatedButton("▶ BOT STARTEN", on_click=start_bot_click, bgcolor=ft.colors.GREEN_800, color=ft.colors.WHITE)

    page.add(
        ft.Row([header], alignment=ft.MainAxisAlignment.CENTER),
        ft.Divider(),
        ft.Row([start_btn, status_text], alignment=ft.MainAxisAlignment.SPACE_BETWEEN),
        ft.Divider(),
        ft.Container(
            content=log_view,
            border=ft.border.all(1, ft.colors.GREY_800),
            border_radius=10,
            padding=10,
            expand=True,
            bgcolor=ft.colors.BLACK
        )
    )

ft.app(target=main)
