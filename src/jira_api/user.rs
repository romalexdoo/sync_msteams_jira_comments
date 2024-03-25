use anyhow::{Context, Result};
use serde::Deserialize;

use super::model::JiraAPIShared;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JiraUser {
    account_id: String,
    display_name: Option<String>,
    #[serde(alias="email")]
    email_address: Option<String>,
}

impl JiraUser {
    pub fn get_display_name (&self) -> Option<String> {
        self.display_name.clone()
    }

    pub async fn get_email(&self, jira_api: &JiraAPIShared) -> Result<Option<String>> {
        let response = jira_api.client
            .get(format!("{}/rest/api/2/user/email", jira_api.config.base_url))
            .basic_auth(&jira_api.config.user, Some(&jira_api.config.token))
            .query(&[("accountId", &self.account_id)])
            .send()
            .await
            .context("Failed to send get user email request")?
            .error_for_status()
            .context("Get user email bad status")?
            .json::<JiraUser>()
            .await
            .context("Parse get user email response")?;
        
        Ok(response.email_address)
    }
}

pub async fn get_jira_user_id(jira_api: &JiraAPIShared, reporter_email: &String) -> Result<String> {
    let mut page = 0;
    let mut result = String::new();

    loop {
        let users = jira_api.client
            .get(format!("{}/rest/api/3/users", jira_api.config.base_url))
            .query(&[("startAt", page * 50), ("maxResults", 50)])
            .basic_auth(&jira_api.config.user, Some(&jira_api.config.token))
            .send()
            .await
            .context("Failed to send get reporter request")?
            .error_for_status()
            .context("Get reporter request bad status")?
            .json::<Vec<JiraUser>>()
            .await
            .context("Parse get reporter response")?;
        
        if users.len() == 0 {
            break;
        }

        let reporter = users
            .iter()
            .find(|u| u.email_address.clone().unwrap_or_default().to_lowercase() == *reporter_email.to_lowercase());

        if reporter.is_some() {
            result = reporter.unwrap().account_id.clone();
            break;
        };

        page += 1;
    };

    Ok(result)
}

