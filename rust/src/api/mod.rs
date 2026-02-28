//! API module - Kraken REST API client and Flutter bridge
//!
//! This module contains:
//! - `rest_client` - Low-level Kraken API with HMAC signing
//! - `simple` - Flutter bridge functions

pub mod rest_client;
pub mod simple;

pub use rest_client::KrakenRestClient;
