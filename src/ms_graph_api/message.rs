use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MsGraphMessage {
    pub id: String,
    pub web_url: Option<String>,
    pub from: MessageFrom,
    pub body: MessageBody,
    pub attachments: Vec<TeamsAttachment>,
    pub subject: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageFrom {
    pub user: Option<MsGraphUser>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageBody {
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamsAttachment {
    pub id: Uuid,
    pub content_url: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MsGraphUser {
    pub id: Uuid,
    pub display_name: Option<String>,
}

impl MsGraphMessage {
    pub async fn get(client: &Client, resource: &String, access_token: &String) -> Result<Self> {
        let message = client
            .get(format!("https://graph.microsoft.com/v1.0/{resource}"))
            .bearer_auth(access_token)
            .send()
            .await
            .context("Failed to send get message request")?
            .error_for_status()
            .context("Get message request bad status")?
            .json::<MsGraphMessage>()
            .await
            .context("Parse get message response")?;

        Ok(message)
    }
}
