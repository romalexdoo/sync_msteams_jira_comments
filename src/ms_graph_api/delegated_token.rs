use anyhow::{ensure, Context, Result};
use reqwest::Client;
use serde::Deserialize;
use tokio::time::{Duration, Instant};

use super::cfg::Config;

#[derive(Deserialize)]
pub struct GrantedToken {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    #[serde(skip, default = "default_instant")]
    expires_at: Instant,
}

fn default_instant() -> Instant {
    Instant::now()
}

impl GrantedToken {
    pub fn new() -> Self {
        Self { access_token: String::new(), refresh_token: String::new(), expires_in: 0, expires_at: default_instant() }
    }
    
    pub async fn set_first_time(&mut self, client: &Client, config: &Config, code: String) -> Result<()> {
        let form = [
            ("client_id", config.client_id.as_str()),
            ("scope", "ChannelMessage.Send ChannelMessage.ReadWrite"),
            ("code", code.as_str()),
            ("redirect_uri", config.oauth_url.as_str()),
            ("grant_type", "authorization_code"),
            // ("client_secret", config.client_secret.as_str()),
        ];

        let _ = self.set(client, config, &form).await?;

        Ok(())
    }

    async fn set(&mut self, client: &Client, config: &Config, form: &[(&str, &str)]) -> Result<(String, u64)> {

        let token = client
            .post(&format!("https://login.microsoftonline.com/{}/oauth2/v2.0/token", config.tenant_id))
            .form(form)
            .send()
            .await
            .context("Failed to send get token request")?
            .error_for_status()
            .context("Get token request bad status")?
            .json::<GrantedToken>()
            .await
            .context("Parse get token response")?;

        self.access_token = token.access_token.clone();
        self.expires_in = token.expires_in;
        self.refresh_token = token.refresh_token;
        self.expires_at = Instant::now() + Duration::from_secs(token.expires_in);

        Ok((token.access_token, token.expires_in))
    }

    pub fn get(&self) -> Result<String> {
        ensure!(!self.access_token.is_empty(), "Token value is empty");
        ensure!(Instant::now() < self.expires_at, "Token expired");
        Ok(self.access_token.clone())
    }

    pub async fn refresh_and_get_expiration_time(&mut self, client: &Client, config: &Config) -> Result<u64> {
        let refresh_token = self.refresh_token.clone();
        let form = [
            ("client_id", config.client_id.as_str()),
            ("scope", "ChannelMessage.Send ChannelMessage.ReadWrite"),
            ("refresh_token", refresh_token.as_str()),
            ("grant_type", "refresh_token"),
            // ("client_secret", config.client_secret.as_str()),
        ];

        let (_, expires_in) = self.set(client, config, &form).await?;

        Ok(expires_in)
    }
}
