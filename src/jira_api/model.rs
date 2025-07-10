use anyhow::{bail, Context as _, Result};
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::utils::get_reqwest_client;

use super::cfg::Config;


pub struct JiraAPI {
    pub(crate) config: Config,
    pub(crate) client: Client,
    pub(crate) users: RwLock<Vec<JiraUser>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JiraUser {
    pub(crate) account_id: String,
    pub(crate) display_name: Option<String>,
    pub(crate) email_address: Option<String>,
}


impl JiraAPI {
    pub fn new(config: Config) -> Result<Self> {
        let jira_api = Self { 
            config,
            client: get_reqwest_client()?,
            users: RwLock::new(Vec::new()),
        };
        Ok(jira_api)
    }

    pub(crate) async fn find_user_by_id(&self, id: &String) -> Result<JiraUser> {
        let user = self
            .users
            .read()
            .await
            .iter()
            .find(|u| u.account_id == *id)
            .map(|u| u.clone());

        if user.is_none() {
            let new_user = self.get_user_from_api_by_id(id).await?;

            self
                .users
                .write()
                .await
                .push(new_user.clone());

            Ok(new_user)
        } else {
            Ok(user.unwrap())
        }
    }

    async fn get_user_from_api_by_id(&self, id: &String) -> Result<JiraUser> {
        let result = self.client
            .get(format!("{}/rest/api/2/user", self.config.base_url))
            .basic_auth(&self.config.user, Some(&self.config.token))
            .query(&[("accountId", id)])
            .send()
            .await
            .context("Failed to send get user email request")?
            .error_for_status()
            .context("Get user email bad status")?
            .json::<JiraUser>()
            .await
            .context("Parse get user email response")?;

        Ok(result)
    }

    pub(crate) async fn get_jira_user_by_email(&self, email: &String) -> Result<Option<JiraUser>> {
        let user = self
            .users
            .read()
            .await
            .iter()
            .find(|u| u.email_address.as_ref().map_or(false, |e| e.to_lowercase() == email.to_lowercase()))
            .map(|u| u.clone());

        if user.is_some() {
            Ok(user)
        } else {
            let maybe_new_user = self.get_user_from_api_by_email(email).await?;

            if let Some(new_user) = maybe_new_user.clone() {
                self
                    .users
                    .write()
                    .await
                    .push(new_user);
            }
        
            Ok(maybe_new_user)
        }
    }

    async fn get_user_from_api_by_email(&self, email: &String) -> Result<Option<JiraUser>> {
        let mut page = 0;
        
        loop {
            let result = self.client
                .get(format!("{}/rest/api/3/users", self.config.base_url))
                .query(&[("startAt", page * 50), ("maxResults", 50)])
                .basic_auth(&self.config.user, Some(&self.config.token))
                .send()
                .await
                .context("Failed to send get reporter request")?;

            if !result.status().is_success() {
                let status = result.status();
                let text = result.text().await.unwrap_or_default();
                bail!("Get reporter request status: {}, text: {}", status, text);
            }

            let users = result
                .json::<Vec<JiraUser>>()
                .await
                .context("Parse get reporter response")?;
            
            if users.len() == 0 {
                break;
            }
    
            let reporter = users
                .iter()
                .find(|u| u.email_address.clone().unwrap_or_default().to_lowercase() == email.to_lowercase())
                .map(|u| u.clone());
    
            if reporter.is_some() {
                return Ok(reporter);
            };
    
            page += 1;
        }

        Ok(None)
    }
}
