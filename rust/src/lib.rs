//! Matrix Quant Core - High-performance trading engine
//!
//! This library provides the core trading functionality for the Matrix Quant
//! cryptocurrency trading bot. It includes:
//!
//! - Kraken REST API client with HMAC-SHA512 authentication
//! - Momentum-based pump detection algorithm
//! - Dynamic trailing stop-loss with step-up levels
//! - Volatility-based regime selection
//! - Flutter bridge for mobile app integration
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    Flutter Mobile App                   │
//! │                   (lib/main.dart)                       │
//! └────────────────────────┬────────────────────────────────┘
//!                          │ flutter_rust_bridge
//! ┌────────────────────────▼────────────────────────────────┐
//! │                   api/simple.rs                         │
//! │              (Bridge Functions)                         │
//! └────────────────────────┬────────────────────────────────┘
//!                          │
//! ┌────────────────────────▼────────────────────────────────┐
//! │                trading/engine.rs                        │
//! │             (TradingEngine Core)                        │
//! │  - detect_pumps()                                       │
//! │  - check_exit()                                         │
//! │  - execute_buy() / execute_sell()                       │
//! └────────────────────────┬────────────────────────────────┘
//!                          │
//! ┌────────────────────────▼────────────────────────────────┐
//! │                api/rest_client.rs                       │
//! │             (Kraken REST API)                           │
//! │  - HMAC-SHA512 signing                                  │
//! │  - Public & Private endpoints                           │
//! └─────────────────────────────────────────────────────────┘
//! ```

pub mod api;
pub mod config;
pub mod trading;

// Re-export main types for library users
pub use config::BotConfig;
pub use trading::engine::TradingEngine;
pub use trading::{OpenTrade, TradeInfo};

// Re-export bridge functions for flutter_rust_bridge codegen
pub use api::simple::*;
