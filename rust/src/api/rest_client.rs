use anyhow::{anyhow, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as Base64};
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde_json::Value;
use sha2::{Digest, Sha256, Sha512};
use std::time::{SystemTime, UNIX_EPOCH};

const API_URL: &str = "https://api.kraken.com";

#[derive(Clone)]
pub struct KrakenRestClient {
    client: Client,
    api_key: String,
    api_secret: String,
}

impl KrakenRestClient {
    pub fn new(api_key: String, api_secret: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            api_secret,
        }
    }

    /// Generate Kraken API signature
    fn get_kraken_signature(&self, url_path: &str, nonce: &str, post_data: &str) -> Result<String> {
        if self.api_secret.is_empty() {
            return Err(anyhow!("API secret is missing"));
        }

        // 1. Decode base64 secret
        let secret_bytes = Base64.decode(&self.api_secret)?;

        // 2. Hash nonce + post_data using SHA256
        let mut sha256 = Sha256::new();
        sha256.update(nonce.as_bytes());
        sha256.update(post_data.as_bytes());
        let hash_digest = sha256.finalize();

        // 3. HMAC-SHA512 of path + sha256_hash using decoded secret
        let mut mac = Hmac::<Sha512>::new_from_slice(&secret_bytes)
            .map_err(|e| anyhow!("HMAC error: {}", e))?;
        
        mac.update(url_path.as_bytes());
        mac.update(&hash_digest);
        let hmac_result = mac.finalize().into_bytes();

        // 4. Base64 encode the result
        Ok(Base64.encode(hmac_result))
    }

    /// Helper to get current nonce
    fn get_nonce() -> String {
        let start = SystemTime::now();
        let since_the_epoch = start.duration_since(UNIX_EPOCH).expect("Time went backwards");
        let nonce = since_the_epoch.as_millis() as u64;
        nonce.to_string()
    }

    /// Generic public request
    pub async fn public_request(&self, endpoint: &str, params: &[(&str, &str)]) -> Result<Value> {
        let url = format!("{}{}", API_URL, endpoint);
        let resp = self.client.get(&url).query(params).send().await?;
        
        let mut json: Value = resp.json().await?;
        if let Some(errs) = json.get("error").and_then(|e| e.as_array()) {
            if !errs.is_empty() {
                return Err(anyhow!("API Error: {:?}", errs));
            }
        }
        
        Ok(json["result"].take())
    }

    /// Generic private request
    pub async fn private_request(&self, endpoint: &str, mut payload: Vec<(&str, String)>) -> Result<Value> {
        if self.api_key.is_empty() {
            return Err(anyhow!("API key is missing"));
        }

        let nonce = Self::get_nonce();
        payload.push(("nonce", nonce.clone()));

        // Encode payload to application/x-www-form-urlencoded
        let post_data = serde_urlencoded::to_string(&payload)?;
        
        let signature = self.get_kraken_signature(endpoint, &nonce, &post_data)?;
        
        let url = format!("{}{}", API_URL, endpoint);
        
        let req = self.client.post(&url)
            .header("API-Key", &self.api_key)
            .header("API-Sign", signature)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(post_data);

        let resp = req.send().await?;
        let mut json: Value = resp.json().await?;
        
        if let Some(errs) = json.get("error").and_then(|e| e.as_array()) {
            if !errs.is_empty() {
                return Err(anyhow!("API Error: {:?}", errs));
            }
        }
        
        Ok(json["result"].take())
    }
}
