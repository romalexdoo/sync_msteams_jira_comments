use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::ms_graph_api::{message::TeamsAttachment, model::MSGraphAPIShared};

use super::{
    attachment::{add_attachments_urls_to_description, find_old_attached_images, replace_attachments, replace_images_in_description, JiraAttachment}, 
    // comment::JiraComment, 
    model::JiraAPIShared,
};


#[derive(Clone, Debug, Deserialize)]
pub struct Issue {
    id: String,
    key: String,
    // #[serde(rename = "self")]
    // url: String,
    fields: Option<IssueFields>,
}

#[derive(Clone, Debug, Deserialize)]
struct IssueFields {
    attachment: Vec<JiraAttachment>,
    description: String,
    // comment: IssueCommentField,
    status: IssueStatus,
    // summary: String,
}

// #[derive(Deserialize)]
// struct IssueCommentField {
//     comments: Vec<JiraComment>,
// }

#[derive(Clone, Debug, Deserialize)]
struct IssueStatus {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserResponse {
    account_id: String,
    email_address: Option<String>,
}


impl Issue {
    pub fn get_id(&self) -> String {
        self.id.clone()
    }
    
    pub fn get_key(&self) -> String {
        self.key.clone()
    }
    
    pub fn get_attachments(&self) -> Option<Vec<JiraAttachment>> {
        self.fields
            .as_ref()
            .map(|f| f.attachment.clone())
    }
    
    pub fn get_description(&self) -> Option<String> {
        self.fields
            .as_ref()
            .map(|f| f.description.clone())
    }

    pub fn get_status(&self) -> String {
        self.fields
            .as_ref()
            .map_or(String::new(), |f| f.status.name.clone())
    }
    
    pub async fn create_or_update (
        jira_api: &JiraAPIShared,
        summary: &String, 
        description: &String, 
        reporter_email: &String, 
        attachments: &Vec<TeamsAttachment>,
        graph_api_token: &String,
        message_url: &String,
        graph_api: &MSGraphAPIShared,
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
                    "key": jira_api.config.project_key
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

        fields.insert(jira_api.config.msteams_link_field_name.clone(), teams_link["teams_link"].clone());

        let reporter_id = get_jira_user_id(jira_api, reporter_email).await.unwrap_or_default();
    
        if !reporter_id.is_empty() {
            let reporter = json!({
                "reporter": {
                    "accountId": reporter_id
                }
            });
    
            fields.insert("reporter".to_string(), reporter["reporter"].clone());
        }
    
        let mut issue = Issue::find(jira_api, message_url, graph_api, message_id).await?;
        let issue_exists = issue.is_some();
    
        if !issue_exists {
            issue = Some(
                    jira_api.client
                        .post(format!("{}/rest/api/2/issue", jira_api.config.base_url))
                        .basic_auth(&jira_api.config.user, Some(&jira_api.config.token))
                        .json(&payload)
                        .send()
                        .await
                        .context("Failed to send create issue request")?
                        .error_for_status()
                        .context("Create request bad status")?
                        .json::<Issue>()
                        .await
                        .context("Parse create issue response")?
                );
        } else {
            let issue = issue.as_mut().unwrap();
            issue.update(jira_api, &payload).await?;
        }
    
        let issue = issue.unwrap();
    
        let old_image_names = find_old_attached_images(&issue.get_description().unwrap_or_default());
        replace_attachments(jira_api, &issue, &old_image_names, &images).await?;
    
        Ok((issue, issue_exists))
    }

    pub async fn find(
        jira_api: &JiraAPIShared, 
        teams_url: &String,
        graph_api: &MSGraphAPIShared,
        message_id: &String,
    ) -> Result<Option<Self>> {
        let jql = format!("project = \"{}\" AND \"MS Teams link[URL Field]\" = \"{}\"", jira_api.config.project_key, teams_url);

        #[derive(Deserialize)]
        struct SearchResponse {
            issues: Vec<Issue>,
        }

        let mut response = jira_api.client
            .get(format!("{}/rest/api/2/search", jira_api.config.base_url))
            .basic_auth(&jira_api.config.user, Some(&jira_api.config.token))
            .query(&[("maxResults", "1"), ("jql", &jql), ("fields", "attachment,description,comment,status,summary")])
            .send()
            .await
            .context("Failed to send search issue request")?
            .error_for_status()
            .context("Search request bad status")?
            .json::<SearchResponse>()
            .await
            .context("Parse search issue response")?;


        if response.issues.len() != 1 {
            return Ok(None)
        }

        let issue = response.issues.pop().unwrap();

        if issue.clone().fields.map_or(false, |i| i.status.is_final()) {
            graph_api
                .reply_to_issue(message_id, &String::from("Извините, но данная задача закрыта. Просим вас завести новую, иначе мы можем пропустить это сообщение"))
                .await?;
        }

        Ok(Some(issue))
    }

    pub async fn update(&self, jira_api: &JiraAPIShared, payload: &Value) -> Result<()> {
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
            .json::<Vec<UserResponse>>()
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

impl IssueStatus {
    pub fn is_final(&self) -> bool {
        self.name.to_lowercase() == "Done".to_lowercase() || self.name.to_lowercase() == "Rejected".to_lowercase()
    }
}
