pub mod rest_client;

use reqwest::Client;
use serde_json::Value;

pub struct KrakenApi {
    pub client: Client,
    pub api_key: String,
    pub api_secret: String,
}

impl KrakenApi {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            api_secret,
        }
    }
}
