#!/usr/bin/env bash
# Wrapper script to run Freqtrade with proper NixOS library loading

# Add NixOS system libraries to LD_LIBRARY_PATH
GCC_LIB=$(nix-build --no-out-link '<nixpkgs>' -A stdenv.cc.cc.lib 2>/dev/null)/lib
export LD_LIBRARY_PATH="$GCC_LIB:${LD_LIBRARY_PATH:-}"

# Disable PIP_USER for venv usage
export PIP_USER=false

# Run freqtrade with all arguments forwarded
exec "$(dirname "$0")/venv/bin/freqtrade" "$@"
