use std::sync::Arc;

use anyhow::Result;
use reqwest::Client;

use crate::utils::get_reqwest_client;

use super::cfg::Config;


pub type JiraAPIShared = Arc<JiraAPI>;

pub struct JiraAPI {
    pub config: Config,
    pub client: Client,
}

impl JiraAPI {
    pub fn new(config: Config) -> Result<Self> {
        let jira_api = Self { 
            config,
            client: get_reqwest_client()?,
        };
        Ok(jira_api)
    }
}
