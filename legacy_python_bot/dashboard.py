#!/usr/bin/env python3
"""
Kraken AutoTrader Command Center
A modern Flask Web Dashboard for controlling the Python bot.
"""
import os
import json
import logging
from pathlib import Path
from flask import Flask, render_template, jsonify, request

app = Flask(__name__)

# Paths
BASE_DIR = Path(__file__).parent
CONFIG_FILE = BASE_DIR / "config.json"
LOG_FILE = BASE_DIR / "autotrader.log"

# Add standard logging
log = logging.getLogger("dashboard")
logging.basicConfig(level=logging.INFO, format="%(asctime)s │ %(message)s")


@app.route('/')
def index():
    """Serves the main dashboard UI."""
    return render_template('index.html')


@app.route('/api/config', methods=['GET'])
def get_config():
    """Returns the current bot configuration."""
    if not CONFIG_FILE.exists():
        return jsonify({"error": "Config file not found"}), 404
        
    try:
        with open(CONFIG_FILE, 'r') as f:
            data = json.load(f)
            # Remove keys for safety before sending to frontend
            if "exchange" in data and "key" in data["exchange"]:
                data["exchange"]["key"] = "**************"
            if "exchange" in data and "secret" in data["exchange"]:
                data["exchange"]["secret"] = "**************"
            if "telegram" in data and "token" in data["telegram"]:
                data["telegram"]["token"] = "**************"
        return jsonify(data)
    except Exception as e:
        return jsonify({"error": str(e)}), 500


@app.route('/api/config', methods=['POST'])
def update_config():
    """Saves non-sensitive config changes from the UI."""
    if not CONFIG_FILE.exists():
        return jsonify({"error": "Config file not found"}), 404
        
    try:
        new_settings = request.json
        
        # Read old config to keep keys safe
        with open(CONFIG_FILE, 'r') as f:
            data = json.load(f)
            
        # Update allowed fields
        if "telegram" in new_settings:
            data["telegram"]["enabled"] = new_settings["telegram"].get("enabled", data["telegram"]["enabled"])
            
        with open(CONFIG_FILE, 'w') as f:
            json.dump(data, f, indent=4)
            
        log.info("UI Update: Config saved successfully.")
        return jsonify({"success": True})
        
    except Exception as e:
        log.error(f"UI Error saving config: {e}")
        return jsonify({"error": str(e)}), 500


@app.route('/api/logs', methods=['GET'])
def get_logs():
    """Returns the last N lines of the autotrader log."""
    lines_to_read = int(request.args.get('lines', 50))
    if not LOG_FILE.exists():
        return jsonify({"lines": ["No log file found yet. Wait for the bot to start..."]})
        
    try:
        with open(LOG_FILE, 'r') as f:
            # Simple tail implementation
            lines = f.readlines()
            tail = lines[-lines_to_read:] if len(lines) > lines_to_read else lines
            return jsonify({"lines": [line.strip() for line in tail]})
    except Exception as e:
        return jsonify({"error": str(e)}), 500


@app.route('/api/stats', methods=['GET'])
def get_stats():
    """Extracts live stats (like active slots) from the latest logs."""
    stats = {"slots": "0/3"}
    if not LOG_FILE.exists():
        return jsonify(stats)
        
    try:
        with open(LOG_FILE, 'r') as f:
            # Get last 100 lines and search backwards for the latest state
            lines = f.readlines()[-100:]
            for line in reversed(lines):
                if "Slots:" in line:
                    # Example format: "──── Zyklus 16 | 15:45:22 | €0.23 | Slots: 1/3 | P/L: €+0.00 ────"
                    try:
                        parts = line.split("|")
                        for p in parts:
                            if "Slots:" in p:
                                stats["slots"] = p.replace("Slots:", "").strip()
                                break
                        break
                    except Exception:
                        pass
        return jsonify(stats)
    except Exception as e:
        return jsonify({"error": str(e)}), 500


if __name__ == '__main__':
    log.info("🚀 Starting Web Dashboard on http://localhost:5000")
    
    # Versuche den Browser nach 1 Sekunde Verzögerung zu öffnen
    import threading
    import time
    import subprocess
    
    def open_browser():
        time.sleep(1.5)
        try:
            # Versuche Vivaldi spezifisch zu öffnen
            subprocess.Popen(['vivaldi', 'http://127.0.0.1:5000'])
            log.info("🌐 Opening Dashboard in Vivaldi...")
        except FileNotFoundError:
            # Fallback wenn Vivaldi nicht im PATH ist
            import webbrowser
            webbrowser.open('http://127.0.0.1:5000')
            log.info("🌐 Opening Dashboard in default browser...")
            
    threading.Thread(target=open_browser, daemon=True).start()
    
    # Setting host='0.0.0.0' allows access from local network if desired, but 127.0.0.1 is safer for now
    app.run(host='127.0.0.1', port=5000, debug=False) # Debug false verhindert doppelten Browser-Start
