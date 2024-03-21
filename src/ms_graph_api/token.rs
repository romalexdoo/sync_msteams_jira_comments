use std::time::{Duration, Instant};

use anyhow::{ensure, Context, Result};
use reqwest::Client;
use serde::Deserialize;

use super::cfg::Config;

pub struct ApplicationToken {
    pub value: String,
    pub expires_at: Instant,
}

impl ApplicationToken {
    pub fn new() -> Self {
        Self { value: String::new(), expires_at: Instant::now() }
    }

    pub fn get(&self) -> Result<String> {
        ensure!(!self.value.is_empty(), "Token value is empty");
        ensure!(Instant::now() < self.expires_at, "Token expired");
        Ok(self.value.clone())
    }

    pub async fn renew(&mut self, client: &Client, config: &Config) -> Result<String> {

        #[derive(Deserialize)]
        struct TokenResponse {
            expires_in: u64,
            access_token: String,
        }
        
        let token = client
            .post(&format!("https://login.microsoftonline.com/{}/oauth2/v2.0/token", config.tenant_id))
            .form(&[
                ("scope", "https://graph.microsoft.com/.default"),
                ("grant_type", "client_credentials"),
                ("client_id", &config.client_id),
                ("client_secret", &config.client_secret),
            ])
            .send()
            .await
            .context("Failed to send get token request")?
            .error_for_status()
            .context("Get token request bad status")?
            .json::<TokenResponse>()
            .await
            .context("Parse get token response")?;

        self.value = token.access_token;
        self.expires_at = Instant::now() + Duration::from_secs(token.expires_in / 2);

        Ok(self.value.clone())
    }
}