use anyhow::{bail, ensure, Context, Result};
use chrono::{DateTime, Utc};
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::cfg::Config;

pub struct Subscription {
    subscription_id: Uuid,
    subscription_secret: Uuid,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NewSubsciptionRequest {
    change_type: String,
    notification_url: String,
    lifecycle_notification_url: String,
    resource: String,
    expiration_date_time: DateTime<Utc>,
    client_state: Uuid,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RenewSubsciptionRequest {
    expiration_date_time: DateTime<Utc>,
}

#[derive(Deserialize)]
struct NewSubsciptionResponse {
    id: Uuid,
}

#[derive(Deserialize)]
struct ActiveSubsciptionResponse {
    value: Vec<NewSubsciptionResponse>,
}

impl Subscription {
    pub(crate) fn new() -> Self {
        Self { subscription_id: Uuid::nil(), subscription_secret: Uuid::nil() }
    }

    pub async fn init(&mut self, client: &Client, config: &Config, access_token: &String, repeated: bool) -> Result<()> {
        let subscription_secret = Uuid::new_v4();
        let response = add_subscription_response(config, client, access_token, &subscription_secret).await?;

        if response.status().is_success() {
            self.subscription_secret = subscription_secret;
            self.subscription_id = response.json::<NewSubsciptionResponse>().await.context("Failed to retrieve subscription ID")?.id;
        } else if response.status() == StatusCode::FORBIDDEN {
            if repeated {
                bail!("Failed to kill active subscription");
            } else {
                kill_active_subscription(client, access_token).await?;
                
                let response = add_subscription_response(config, client, access_token, &subscription_secret).await?;
                ensure!(response.status().is_success(), response.text().await?);

                self.subscription_secret = subscription_secret;
                self.subscription_id = response.json::<NewSubsciptionResponse>().await.context("Failed to retrieve subscription ID")?.id;
            };
        } else {
            bail!(response.text().await?)
        }

        let auth_url = format!("https://login.microsoftonline.com/{}/oauth2/v2.0/authorize?client_id={}&scope=offline_access%20ChannelMessage.Send%20ChannelMessage.ReadWrite&response_type=code&redirect_uri={}&response_mode=form_post&state={}", config.tenant_id, config.client_id, config.oauth_url, subscription_secret);
        println!("{auth_url}");
        
        let content = format!("Please, go to email below<BR><a href=\"{}\">{}</a>", auth_url, auth_url);
        
        let payload = json!({
            "message": {
                "subject": "Jira vs Teams authentication link",
                "body": {
                    "contentType": "html",
                    "content": content,
                },
                "toRecipients": [
                    {
                        "emailAddress": {
                            "address": config.teams_user
                        }
                    }
                ]
            }
        });
        
        client
            .post(format!("https://graph.microsoft.com/v1.0/users/{}/sendMail", config.teams_user))
            .bearer_auth(access_token)
            .json(&payload)
            .send()
            .await
            .context("Failed to send email")?;

        Ok(())
    }

    pub(crate) async fn renew(&mut self, client: &Client, access_token: &String, subsciption_id: &String) -> Result<()> {
        let req = RenewSubsciptionRequest {
            expiration_date_time: Utc::now() + chrono::Duration::try_hours(3).unwrap(),
        };

        client
            .patch(format!("https://graph.microsoft.com/v1.0/subscriptions/{}", subsciption_id))
            .bearer_auth(access_token)
            .json(&req)
            .send()
            .await
            .context("Failed to send renew subscription request")?
            .error_for_status()
            .context("Renew subscription request bad status")?;

        Ok(())
    }

    pub(crate) fn check_client_secret(&self, secret: &String) -> Result<()> {
        let secret_uuid = Uuid::try_parse(secret.as_str())?;
        ensure!(secret_uuid == self.subscription_secret, "Incorrect secret");
        Ok(())
    }
}

async fn kill_active_subscription(client: &Client, access_token: &String) -> Result<()> {
    let response = client
        .get("https://graph.microsoft.com/v1.0/subscriptions/")
        .bearer_auth(access_token)
        .send()
        .await
        .context("Failed to send get subscription request")?;

    if response.status().is_success() {
        if let Ok(s) = response.json::<ActiveSubsciptionResponse>().await {
            if let Some(r) = s.value.first() {
                client
                    .delete(format!("https://graph.microsoft.com/v1.0/subscriptions/{}", r.id))
                    .bearer_auth(access_token)
                    .send()
                    .await
                    .context("Failed to send delete subscription request")?
                    .error_for_status()
                    .context("Delete subscription request bad status")?;
            }
        }
    }

    Ok(())
}

async fn add_subscription_response(config: &Config, client: &Client, access_token: &String, subscription_secret: &Uuid) -> Result<Response> {
    let req = NewSubsciptionRequest {
        change_type: String::from("created,updated"),
        notification_url: config.notification_url.clone(),
        lifecycle_notification_url: config.lifecycle_notification_url.clone(),
        resource: format!("/teams/{}/channels/{}/messages", config.group_id, config.channel_id),
        expiration_date_time: Utc::now() + chrono::Duration::try_hours(3).unwrap(),
        client_state: subscription_secret.clone(),
    };

    client
        .post("https://graph.microsoft.com/v1.0/subscriptions")
        .bearer_auth(access_token)
        .json(&req)
        .send()
        .await
        .context("Failed to send new subscription request")
}