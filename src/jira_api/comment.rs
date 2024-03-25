use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::ms_graph_api::{message::TeamsAttachment, model::MSGraphAPIShared};

use super::{
    attachment::{add_attachments_urls_to_description, find_old_attached_images, replace_attachments, replace_images_in_description}, 
    issue::Issue, 
    model::JiraAPIShared, user::{get_jira_user_id, JiraUser},
};

const PROPERTY_KEY: &str = "teams_id";


#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JiraComment {
    pub id: String,
    pub body: String,
    pub update_author: JiraUser,
    pub properties: Option<Vec<JiraCommentProperty>>,
}

#[derive(Debug, Deserialize)]
pub struct JiraCommentProperty {
    pub key: String,
    pub value: Option<JiraCommentPropertyValue>,
}

#[derive(Debug, Deserialize)]
pub struct JiraCommentPropertyValue {
    pub teams_id: String,
}

impl JiraComment {
    pub async fn create_or_update (
        jira_api: &JiraAPIShared,
        description: &String, 
        author_email: &String, 
        attachments: &Vec<TeamsAttachment>,
        graph_api_token: &String,
        message_url: &String,
        reply_id: &String,
        graph_api: &MSGraphAPIShared,
        message_id: &String,
    ) -> Result<Self> {
        // let description = htmltoadf::convert_html_str_to_adf_str(description.clone());
        // let description_json: Value = serde_json::from_str(description.as_str()).context("Failed to parse description JSON")?;
        let mut description_v2 = html2md::parse_html(description);

        add_attachments_urls_to_description(&mut description_v2, attachments);
        let images = replace_images_in_description(&mut description_v2, graph_api_token).await?;

        let mut author_id = get_jira_user_id(jira_api, author_email).await.unwrap_or_default();
        if author_id.len() == 0 {
            author_id = author_email.clone();
        }

        description_v2 = format!("On behalf of [~accountid:{}]:\n\n{}", author_id, description_v2);
    

        let payload = json!({
            "body": description_v2.clone(),
            "properties": [
                {
                    "key": PROPERTY_KEY,
                    "value": {
                        "teams_id": reply_id
                    }
                }
            ]
        });

        let issue = match Issue::find(jira_api, &message_url, graph_api, message_id).await? {
            Some(i) => i,
            None => bail!("Issue not found"),
        };

        let mut comment = JiraComment::find(jira_api, &issue.get_id(), reply_id).await?;
        let comment_body = comment.as_ref().map(|com| com.body.clone()).unwrap_or_default();
    
        if comment.is_none() {
            comment = Some(
                    jira_api.client
                        .post(format!("{}/rest/api/2/issue/{}/comment", jira_api.config.base_url, issue.get_id()))
                        .basic_auth(&jira_api.config.user, Some(&jira_api.config.token))
                        .json(&payload)
                        .send()
                        .await
                        .context("Failed to send create comment request")?
                        .error_for_status()
                        .context("Create comment request bad status")?
                        .json::<JiraComment>()
                        .await
                        .context("Parse create comment response")?
                );
        } else {
            let comment = comment.as_mut().unwrap();
            comment.update(jira_api, &issue.get_id(), &payload).await?;
        }
    
        let comment = comment.unwrap();
    
        let old_image_names = find_old_attached_images(&comment_body);
        replace_attachments(jira_api, &issue, &old_image_names, &images).await?;
    
        Ok(comment)
    }

    pub async fn find(jira_api: &JiraAPIShared, issue_id: &String, reply_id: &String) -> Result<Option<Self>> {

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SearchResponse {
            comments: Vec<JiraComment>,
        }

        let search_response = jira_api.client
            .get(format!("{}/rest/api/2/issue/{}/comment", jira_api.config.base_url, issue_id))
            .basic_auth(&jira_api.config.user, Some(&jira_api.config.token))
            .query(&[("expand", "properties"), ("orderBy", "-created")])
            .send()
            .await
            .context("Failed to get comments issue request")?
            .error_for_status()
            .context("Get comments request bad status")?
            .json::<SearchResponse>()
            .await
            .context("Parse get comments response")?;

        let result = search_response.comments
            .into_iter()
            .find(|r| {
                    let value = r.properties
                        .as_ref()
                        .map(|properties| 
                                properties
                                    .iter()
                                    .find(|p| p.key == String::from("teams_id"))
                                    .map(|p| p.value.as_ref().map(|v| v.teams_id.as_ref()))
                            )
                        .flatten()
                        .flatten()
                        .unwrap_or_default();
                    value == *reply_id
                }
            );

        Ok(result)
    }

    pub async fn update(&self, jira_api: &JiraAPIShared, issue_id: &String, payload: &Value) -> Result<()> {
        jira_api.client
            .put(format!("{}/rest/api/2/issue/{}/comment/{}", jira_api.config.base_url, issue_id, self.id))
            .basic_auth(&jira_api.config.user, Some(&jira_api.config.token))
            .json(&payload)
            .send()
            .await
            .context("Failed to send update comment request")?
            .error_for_status()
            .context("Update comment request bad status")?;

        Ok(())
    }

    pub fn get_reply_id(&self) -> Option<String> {
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

    pub async fn add_reply_id(&self, jira_api: &JiraAPIShared, reply_id: &String) -> Result<()> {
        let payload = json!({
            "teams_id": reply_id
        });
        
        jira_api.client
            .put(format!("{}/rest/api/2/comment/{}/properties/{}", jira_api.config.base_url, self.id, PROPERTY_KEY))
            .basic_auth(&jira_api.config.user, Some(&jira_api.config.token))
            .json(&payload)
            .send()
            .await
            .context("Failed to send set property request")?
            .error_for_status()
            .context("Set property request bad status")?;

        Ok(())
    }
}