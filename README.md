# Binance FIX Trading Bot in Rust

A real-time trading bot that connects to Binance's FIX API endpoints for market data and order execution. The bot implements the FIX 4.4 protocol for authenticated trading operations.

## Features

- **FIX Protocol Implementation**: Custom codec for parsing and encoding FIX messages
- **Dual Connection Architecture**: 
  - Market data stream for real-time price feeds
  - Order execution session for trade management
- **Ed25519 Authentication**: Cryptographic signature-based authentication
- **Order Management**: Support for new orders, cancellations, and execution reports
- **Market Data**: Real-time market data subscription with heartbeat handling

## Architecture

The bot runs two concurrent sessions:
1. **Market Data Client**: Connects to Binance's FIX market data endpoint
2. **Order Execution Client**: Handles order placement and execution via FIX protocol

Both sessions share a common strategy state for coordinated trading decisions.

## Configuration

Set environment variables for Binance FIX API credentials:
- `BINANCE_API_KEY`
- `BINANCE_PRIVATE_KEY_BASE64`
- `BINANCE_TARGET_COMP_ID`
- `BINANCE_MD_HOSTNAME`
- `BINANCE_OE_HOSTNAME`
- `BINANCE_PORT`

## Running

```bash
cargo run
```

The bot will establish both FIX connections and begin processing market data while ready to execute trades based on the sample strategy.
