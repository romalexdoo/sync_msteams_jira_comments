use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Deserializer};
use serde_json::{json, Value};

use crate::{jira_api::model::JiraAPI, ms_graph_api::message::TeamsAttachment, server::server::AppStateShared};

use super::{
    attachment::{add_attachments_urls_to_description, find_old_attached_images, replace_attachments, replace_images_in_description, JiraAttachment}, 
    model::JiraUser,
};


#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Issue {
    id: String,
    key: String,
    // #[serde(rename = "self")]
    // url: String,
    #[serde(deserialize_with = "deserialize_issue_fields")]
    fields: Option<IssueFields>,
}

#[derive(Clone, Debug, Deserialize)]
struct IssueFields {
    attachment: Vec<JiraAttachment>,
    description: String,
    assignee: Option<JiraUser>,
    // comment: IssueCommentField,
    status: IssueStatus,
    // summary: String,
    teams_link: Option<String>,
}


// #[derive(Deserialize)]
// struct IssueCommentField {
//     comments: Vec<JiraComment>,
// }

#[derive(Clone, Debug, Deserialize)]
struct IssueStatus {
    name: String,
}

impl Issue {
    pub(crate) fn get_id(&self) -> String {
        self.id.clone()
    }
    
    pub(crate) fn get_key(&self) -> String {
        self.key.clone()
    }
    
    pub(crate) fn get_attachments(&self) -> Option<Vec<JiraAttachment>> {
        self.fields
            .as_ref()
            .map(|f| f.attachment.clone())
    }
    
    pub(crate) fn get_description(&self) -> Option<String> {
        self.fields
            .as_ref()
            .map(|f| f.description.clone())
    }

    pub(crate) fn get_status(&self) -> Option<String> {
        self.fields
            .as_ref()
            .map(|f| f.status.name.clone())
    }

    pub(crate) fn final_status(&self) -> bool {
        self.fields
            .as_ref()
            .map_or(false, |f| f.status.is_final())
    }

    pub(crate) fn get_teams_link(&self) -> Option<String> {
        self.fields
            .as_ref()
            .and_then(|f| f.teams_link.clone())
    }
    
    pub(crate) fn get_assignee_name(&self) -> Option<String> {
        self.fields
            .as_ref()
            .and_then(|f| 
                f.assignee.as_ref().and_then(|a| 
                    a.display_name.as_ref().map(|dn| 
                        dn.clone()
                    )
                )
            )
    }
    
    pub(crate) async fn create_or_update (
        state_shared: AppStateShared,
        summary: &String, 
        description: &String, 
        reporter_email: &String, 
        attachments: &Vec<TeamsAttachment>,
        graph_api_token: &String,
        message_url: &String,
        message_id: &String,
    ) -> Result<(Self, bool)> {
        let mut summary = summary.clone();
    
        if summary.len() == 0 {
            summary = format!("New issue from {reporter_email}");
        }
    
        // let description = htmltoadf::convert_html_str_to_adf_str(description.clone());
        // let description_json: Value = serde_json::from_str(description.as_str()).context("Failed to parse description JSON")?;
        let mut description_v2 = html2md::parse_html(description);

        add_attachments_urls_to_description(&mut description_v2, attachments);
        let images = replace_images_in_description(&mut description_v2, graph_api_token).await?;
    
        let mut payload = json!({
            "fields": {
                "project": {
                    "key": state_shared.jira.config.project_key
                },
                "summary": summary,
                "description": description_v2.clone(),
                "issuetype": {
                    "name": "Task"
                }
            }
        });
    
        let fields = payload["fields"].as_object_mut().unwrap(); // Safe to unwrap here since we know it exists

        let teams_link = json!({
            "teams_link": message_url
        });

        fields.insert(state_shared.jira.config.msteams_link_field_name.clone(), teams_link["teams_link"].clone());

        let reporter_id = state_shared.jira
            .get_jira_user_by_email(reporter_email).await?.map(|u| u.account_id)
            .unwrap_or_default();
    
        if !reporter_id.is_empty() {
            let reporter = json!({
                "reporter": {
                    "accountId": reporter_id
                }
            });
    
            fields.insert("reporter".to_string(), reporter["reporter"].clone());
        }
    
        
        let mut maybe_issue = Issue::find(state_shared.clone(), message_url, message_id).await?;
        let issue_exists = maybe_issue.is_some();
    
        if !issue_exists {
            #[derive(Deserialize)]
            struct CreateIssueResponse {
                id: String,
            }

            let result = state_shared.jira.client
                        .post(format!("{}/rest/api/2/issue", state_shared.jira.config.base_url))
                        .basic_auth(&state_shared.jira.config.user, Some(&state_shared.jira.config.token))
                        .json(&payload)
                        .send()
                        .await
                        .context("Failed to send create issue request")?
                        .error_for_status()
                        .context("Create request bad status")?
                        .json::<CreateIssueResponse>()
                        .await
                        .context("Parse create issue response")?;
            
            maybe_issue = Issue::get_issue(&state_shared.jira, &result.id).await.ok();
        } else {
            let issue = maybe_issue.as_mut().unwrap();
            issue.update(&state_shared.jira, &payload).await?;
        }
    
        let issue = maybe_issue.ok_or(anyhow!("Failed to get created issue"))?;

        let old_image_names = find_old_attached_images(&issue.get_description().unwrap_or_default());

        replace_attachments(&state_shared.jira, &issue, &old_image_names, &images).await.context("Failed to replace attachments")?;
    
        Ok((issue, issue_exists))
    }

    pub(crate) async fn find(
        state_shared: AppStateShared, 
        teams_url: &String,
        message_id: &String,
    ) -> Result<Option<Self>> {
        let jql = format!("project = \"{}\" AND \"{}\" = \"{}\"", state_shared.jira.config.project_key, state_shared.jira.config.msteams_link_field_jql_name, teams_url);

        #[derive(Deserialize)]
        struct SearchResponse {
            issues: Vec<Issue>,
        }

        let result = state_shared.jira.client
            .get(format!("{}/rest/api/2/search/jql", state_shared.jira.config.base_url))
            .basic_auth(&state_shared.jira.config.user, Some(&state_shared.jira.config.token))
            .query(&[("maxResults", "1"), ("jql", &jql), ("fields", "*all")])
            .send()
            .await
            .context("Failed to send search issue request")?;

        if !result.status().is_success() {
            let status = result.status();
            let text = result.text().await.unwrap_or_default();
            bail!("Issue search request bad status: {}, text: {}", status, text);
        }

        let mut response = result
            .json::<SearchResponse>()
            .await
            .context("Parse search issue response")?;

        if response.issues.len() != 1 {
            return Ok(None)
        }

        let issue = response.issues.pop().unwrap();

        if issue.clone().fields.map_or(false, |i| i.status.is_final()) {
            state_shared.microsoft
                .reply_to_issue(message_id, &String::from("Извините, но данная задача закрыта. Просим вас завести новую, иначе мы можем пропустить это сообщение"))
                .await?;
        }

        Ok(Some(issue))
    }

    pub(crate) async fn get_issue(
        jira_api: &JiraAPI, 
        issue_id: &String,
    ) -> Result<Self> {
        let issue = jira_api.client
            .get(format!("{}/rest/api/2/issue/{}", jira_api.config.base_url, issue_id))
            .basic_auth(&jira_api.config.user, Some(&jira_api.config.token))
            .send()
            .await
            .context("Failed to send get issue request")?
            .error_for_status()
            .context("Get request bad status")?
            .json::<Issue>()
            .await
            .context("Parse get issue response")?;

        Ok(issue)
    }

    pub(crate) async fn update(&self, jira_api: &JiraAPI, payload: &Value) -> Result<()> {
        jira_api.client
            .put(format!("{}/rest/api/2/issue/{}", jira_api.config.base_url, self.id))
            .basic_auth(&jira_api.config.user, Some(&jira_api.config.token))
            .json(&payload)
            .send()
            .await
            .context("Failed to send issue update request")?
            .error_for_status()
            .context("Issue update request bad status")?;

        Ok(())
    }
}

impl IssueStatus {
    pub(crate) fn is_final(&self) -> bool {
        self.name.to_lowercase() == "Done".to_lowercase() || self.name.to_lowercase() == "Rejected".to_lowercase()
    }
}

fn deserialize_issue_fields<'de, D>(deserializer: D) -> Result<Option<IssueFields>, D::Error>
where
    D: Deserializer<'de>,
{
    let env_var_name = std::env::var("JIRA_MSTEAMS_LINK_FIELD_NAME").unwrap_or(String::from("teamsLink"));
    let v: Value = Deserialize::deserialize(deserializer)?;

    if let Some(map) = v.as_object() {
        let mut fields_map = serde_json::Map::new();
        for (k, v) in map {
            // For every key in the JSON, if it matches the dynamic `teams_link` field,
            // remap it to "teams_link". Otherwise, use the key as is.
            if k == &env_var_name {
                fields_map.insert("teams_link".to_string(), v.clone());
            } else {
                fields_map.insert(k.clone(), v.clone());
            }
        }
        let fields: IssueFields = serde_json::from_value(Value::Object(fields_map))
            .map_err(serde::de::Error::custom)?;
        Ok(Some(fields))
    } else {
        Err(serde::de::Error::custom("expected a map for fields"))
    }
}