# Matrix Quant Core v2.0

Rust trading daemon for Kraken. Uses RSI mean-reversion on the most liquid EUR pairs.

## Strategy

Instead of chasing pumps (buying after the move), the bot buys oversold bounces:

1. Polls all EUR tickers every 2 minutes, ranks top 30 by volume
2. Builds price history, calculates RSI(30), EMA(20), ATR(20)
3. Buys when RSI crosses up from oversold (<30) with volume confirmation
4. Exits on RSI overbought (>72), ATR-based trailing stop, or time stop (72h)

## Setup

```bash
cp config.example.json config.json
# Edit config.json with your Kraken API keys
```

## Build & Run

```bash
cd rust
# NixOS:
nix-shell -p gcc --run "cargo build --release"
# Other Linux:
cargo build --release

nohup ./target/release/matrix_quant_core > /tmp/matrix_quant.log 2>&1 &
tail -f /tmp/matrix_quant.log
```

## Config

| Field | Default | Description |
|-------|---------|-------------|
| `max_open_trades` | 2 | Max concurrent positions |
| `exchange.key` | - | Kraken API key |
| `exchange.secret` | - | Kraken API secret |
