#!/usr/bin/env bash
# Kraken Autotrader Startscript
# Setzt die NixOS Library-Pfade für numpy/pandas

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

export LD_LIBRARY_PATH="/nix/store/xc0ga87wdclrx54qjaryahkkmkmqi9qz-gcc-15.2.0-lib/lib:/nix/store/c2qsgf2832zi4n29gfkqgkjpvmbmxam6-zlib-1.3.1/lib:${LD_LIBRARY_PATH}"

exec "$SCRIPT_DIR/venv/bin/python3" "$SCRIPT_DIR/autotrader.py" "$@"
