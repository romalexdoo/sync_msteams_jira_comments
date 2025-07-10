use adf2html::document::Document;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::json;

use super::model::JiraAPI;

const PROPERTY_KEY: &str = "teams_id";


#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JiraCommentV3 {
    pub(crate) id: String,
    pub(crate) body: Document,
    // pub(crate) update_author: JiraUser,
    pub(crate) properties: Option<Vec<JiraCommentProperty>>,
    pub(crate) rendered_body: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct JiraCommentProperty {
    pub(crate) key: String,
    pub(crate) value: Option<JiraCommentPropertyValue>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct JiraCommentPropertyValue {
    pub(crate) teams_id: String,
}

impl JiraCommentV3 {
    // pub(crate) async fn find(jira_api: &JiraAPIShared, issue_id: &String, reply_id: &String) -> Result<Option<Self>> {

    //     #[derive(Deserialize)]
    //     #[serde(rename_all = "camelCase")]
    //     struct SearchResponse {
    //         comments: Vec<JiraCommentV3>,
    //     }

    //     let search_response = jira_api.client
    //         .get(format!("{}/rest/api/3/issue/{}/comment", jira_api.config.base_url, issue_id))
    //         .basic_auth(&jira_api.config.user, Some(&jira_api.config.token))
    //         .query(&[("expand", "properties,renderedBody"), ("orderBy", "-created")])
    //         .send()
    //         .await
    //         .context("Failed to get comments issue request")?
    //         .error_for_status()
    //         .context("Get comments request bad status")?
    //         .json::<SearchResponse>()
    //         .await
    //         .context("Parse get comments response")?;

    //     let result = search_response.comments
    //         .into_iter()
    //         .find(|r| {
    //                 let value = r.properties
    //                     .as_ref()
    //                     .map(|properties| 
    //                             properties
    //                                 .iter()
    //                                 .find(|p| p.key == String::from("teams_id"))
    //                                 .map(|p| p.value.as_ref().map(|v| v.teams_id.as_ref()))
    //                         )
    //                     .flatten()
    //                     .flatten()
    //                     .unwrap_or_default();
    //                 value == *reply_id
    //             }
    //         );

    //     Ok(result)
    // }

    pub(crate) fn get_reply_id(&self) -> Option<String> {
        Some(
            self
                .properties
                .as_ref()?
                .iter()
                .find(|p| p.key == PROPERTY_KEY.to_string())?
                .value
                .as_ref()?
                .teams_id
                .clone()
            )
    }

    pub(crate) async fn add_reply_id(&self, jira_api: &JiraAPI, reply_id: &String) -> Result<()> {
        let payload = json!({
            "teams_id": reply_id
        });

        jira_api.client
            .put(format!("{}/rest/api/3/comment/{}/properties/{}", jira_api.config.base_url, self.id, PROPERTY_KEY))
            .basic_auth(&jira_api.config.user, Some(&jira_api.config.token))
            .json(&payload)
            .send()
            .await
            .context("Failed to send set property request")?
            .error_for_status()
            .context("Set property request bad status")?;

        Ok(())
    }

    pub(crate) async fn get(jira_api: &JiraAPI, issue_id: &String, comment_id: &String) -> Result<Self> {
        Ok(
            jira_api.client
                .get(format!("{}/rest/api/3/issue/{}/comment/{}", jira_api.config.base_url, issue_id, comment_id))
                .basic_auth(&jira_api.config.user, Some(&jira_api.config.token))
                .query(&[("expand", "properties,renderedBody")])
                .send()
                .await
                .context("Failed to get comments issue request")?
                .error_for_status()
                .context("Get comments request bad status")?
                .json::<Self>()
                .await
                .context("Parse get comments response")?
        )
    }
}
