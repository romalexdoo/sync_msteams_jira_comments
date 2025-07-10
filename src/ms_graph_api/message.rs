use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MsGraphMessage {
    pub(crate) id: String,
    pub(crate) web_url: Option<String>,
    pub(crate) from: MessageFrom,
    pub(crate) body: MessageBody,
    pub(crate) attachments: Vec<TeamsAttachment>,
    pub(crate) subject: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MessageFrom {
    pub(crate) user: Option<MsGraphUser>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MessageBody {
    pub(crate) content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TeamsAttachment {
    // pub(crate) id: Uuid,
    pub(crate) content_url: String,
    pub(crate) name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MsGraphUser {
    pub(crate) id: Uuid,
    // pub(crate) display_name: Option<String>,
}

impl MsGraphMessage {
    pub(crate) async fn get(client: &Client, resource: &String, access_token: &String) -> Result<Self> {
        let reply = client
            .get(format!("https://graph.microsoft.com/v1.0/{resource}"))
            .bearer_auth(access_token)
            .send()
            .await
            .context("Failed to send get message request")?
            .error_for_status()
            .context("Get message request bad status")?;

        let message = reply.json::<Value>().await
            .context("Parse get message response to Value")?;

        println!("Message: {}", message.to_string());

        let message = serde_json::from_value::<MsGraphMessage>(message)
            .context("Parse get message response to MsGraphMessage")?;
        
        // let message = client
        //     .get(format!("https://graph.microsoft.com/v1.0/{resource}"))
        //     .bearer_auth(access_token)
        //     .send()
        //     .await
        //     .context("Failed to send get message request")?
        //     .error_for_status()
        //     .context("Get message request bad status")?
        //     .json::<MsGraphMessage>()
        //     .await
        //     .context("Parse get message response")?;

        Ok(message)
    }
}
