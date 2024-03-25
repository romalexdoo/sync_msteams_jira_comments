use anyhow::{ensure, Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use tokio::{
    sync::{Mutex, RwLock},
    time::{sleep, Duration},
};
use uuid::Uuid;
use std::sync::Arc;

use crate::utils::get_reqwest_client;

use super::{cfg::Config, message::MsGraphMessage};
use super::delegated_token::GrantedToken;
use super::subscription::Subscription;
use super::token::ApplicationToken;

pub type MSGraphAPIShared = Arc<MSGraphAPI>;

pub struct MSGraphAPI {
    pub state: Mutex<MSGraphAPIState>,
    pub config: Config,
    pub client: Client,
    pub granted_token: RwLock<GrantedToken>,
}

pub struct MSGraphAPIState {
    pub token: ApplicationToken,
    pub subscription: Subscription,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserResponse {
    mail: String,
}

impl MSGraphAPIState {
    fn new() -> Self {
        Self {
            token: ApplicationToken::new(),
            subscription: Subscription::new(),
        }
    }
}

impl MSGraphAPI {
    pub fn new(config: Config) -> Result<Self> {
        let graph_api = Self { 
            config,
            state: Mutex::new(MSGraphAPIState::new()),
            client: get_reqwest_client()?,
            granted_token: RwLock::new(GrantedToken::new()),
        };
        Ok(graph_api)
    }

    pub async fn get_user_email(&self, access_token: &String, user_id: Uuid) -> Result<String> {
        let user = self.client
            .get(format!("https://graph.microsoft.com/v1.0/users/{}", user_id.to_string()))
            .bearer_auth(access_token)
            .send()
            .await
            .context("Failed to send get user mail request")?
            .error_for_status()
            .context("Get user mail request bad status")?
            .json::<UserResponse>()
            .await
            .context("Parse get user mail response")?;

        Ok(user.mail)
    }

    pub async fn set_delegated_token(&self, code: String) -> Result<()> {
        let mut tx = self.granted_token.write().await;
        tx.set_first_time(&self.client, &self.config, code).await
    }

    pub async fn manage_granted_token(&self) -> Result<()> {
        let mut backoff_time: u64 = 1;
        let mut token_is_empty = true;
    
        loop {
            sleep(Duration::from_secs(backoff_time)).await;
            if token_is_empty {
                token_is_empty = self.granted_token.read().await.get().unwrap_or_default().is_empty();
            }

            if !token_is_empty {
                match self.granted_token.write().await.refresh_and_get_expiration_time(&self.client, &self.config).await {
                    Ok(expires_in) => {
                        backoff_time = expires_in / 2;
                    },
                    Err(_) => {
                        backoff_time /= 2; // Exponential backoff
                        ensure!(backoff_time > 1, "Too many retries");
                    }
                }
            }
        }
    }

    pub async fn reply_to_issue(&self, message_id: &String, reply_body: &String) -> Result<MsGraphMessage> {
        let token = self.granted_token.read().await.get()?;

        let payload = json!(
            {
                "body": {
                    "contentType": "html",
                    "content": reply_body
                }
            }        
        );

        let response = self.client
            .post(format!("https://graph.microsoft.com/v1.0/teams/{}/channels/{}/messages/{}/replies", self.config.group_id, self.config.channel_id, message_id))
            .bearer_auth(token)
            .json(&payload)
            .send()
            .await
            .context("Failed to send reply to issue request")?
            .error_for_status()
            .context("Reply to issue request bad status")?
            .json::<MsGraphMessage>()
            .await
            .context("Parse reply to issue response")?;

        Ok(response)
    }

    pub async fn edit_reply(&self, message_id: &String, reply_id: &String, reply_body: &String) -> Result<()> {
        let token = self.granted_token.read().await.get()?;

        let payload = json!(
            {
                "body": {
                    "contentType": "html",
                    "content": reply_body
                }
            }        
        );

        self.client
            .patch(format!("https://graph.microsoft.com/v1.0/teams/{}/channels/{}/messages/{}/replies{}", self.config.group_id, self.config.channel_id, message_id, reply_id))
            .bearer_auth(token)
            .json(&payload)
            .send()
            .await
            .context("Failed to send reply edit request")?
            .error_for_status()
            .context("Reply edit request bad status")?;

        Ok(())
    }
}