use anyhow::{ensure, Context, Result};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header::HeaderMap, HeaderName, StatusCode};
use axum::response::Result as ApiResult;
use axum::Extension;
use hmac::{Hmac, Mac};
use markdown_to_html_parser::parse_markdown;
use serde::Deserialize;
use serde_json::Value;
use sha2::Sha256;
type HmacSha256 = Hmac<Sha256>;

use crate::jira_api::comment::JiraComment;
use crate::jira_api::issue::Issue;
use crate::jira_api::model::JiraAPIShared;
use crate::ms_graph_api::model::MSGraphAPIShared;
use crate::server::error::{Context as ApiContext, Error as ApiError};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueRequest {
    pub issue: Issue,
    pub changelog: ChangeLog,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentRequest {
    pub comment: JiraComment,
    pub issue: IssueId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueId {
    pub id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeLog {
    pub items: Vec<ChangeLogItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeLogItem {
    pub field: String,
    pub to_string: String,
}

#[derive(Debug)]
struct Signature {
    method: String,
    value: String,
}

pub async fn handler(
    Extension(jira_api): Extension<JiraAPIShared>,
    State(graph_api): State<MSGraphAPIShared>,
    headers: HeaderMap,
    payload: axum::body::Bytes,
) -> ApiResult<StatusCode, ApiError> {
    let signature = get_signature_from_headers(headers)
        .map_err(ApiError::c500)
        .context("Failed to get signature")?;

    validate_signature(&payload, &jira_api.config.secret, &signature)
        .map_err(ApiError::c500)
        .context("Failed to validate signature")?;

    let json_payload = serde_json::from_slice::<Value>(&payload)
        .map_err(ApiError::c500)
        .context("Failed to deserialize payload")?;
println!("1");
    let webhook_event = &json_payload
        .get("webhookEvent")
        .and_then(|t| t.as_str())
        .unwrap_or_default();
println!("2");

    match *webhook_event {
        "comment_created" | "comment_updated" => { 
                parse_comment(payload, &jira_api, &graph_api).await.map_err(ApiError::c500).context("Failed to parse comment")?; 
            },
        _ => { 
                parse_issue(payload, &graph_api).await.map_err(ApiError::c500).context("Failed to parse issue")?;
            },
    }
println!("3");

    Ok(StatusCode::OK)
}

fn validate_signature(payload: &Bytes, secret: &String, signature: &Signature) -> Result<()> {
    ensure!(
        signature.method == "sha256".to_string(),
        "Wrong method, expected: sha256, got: {}",
        signature.method
    );

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())?;
    mac.update(payload);
    let result = hex::encode(mac.finalize().into_bytes());

    ensure!(result == signature.value, "Wrong signature");

    Ok(())
}

fn extract_message_id_from_url(url: String) -> Option<String> {
    let start_pos = url.rfind('/')? + 1;
    let end_pos = url[start_pos..].find('?')? + start_pos;
    Some(url[start_pos..end_pos].to_string())
}

fn get_signature_from_headers(headers: HeaderMap) -> Result<Signature> {
    ensure!(headers.len() > 0, "Headers not present in request");

    let header = headers
        .get(HeaderName::from_static("x-hub-signature"))
        .context("Failed to get signature header")?
        .to_str()
        .context("Failed to get signature header")?;

    let parts: Vec<&str> = header.split("=").collect();
    ensure!(parts.len() == 2, "Incorrect signature header");

    Ok(Signature {
        method: parts[0].to_string(),
        value: parts[1].to_string(),
    })
}

async fn parse_comment(payload: Bytes, jira_api: &JiraAPIShared, graph_api: &MSGraphAPIShared) -> Result<()> {
    let request = serde_json::from_slice::<CommentRequest>(&payload)
        .context("Failed to deserialize payload")?;
println!("2-1");

    let author_email = request.comment.update_author.get_email(jira_api).await.context("Failed to get author email")?.unwrap_or_default();
println!("2-2");

    if author_email == jira_api.config.user {
        return Ok(());
    }
println!("2-3");
println!("{}", std::env::var("JIRA_MSTEAMS_LINK_FIELD_NAME").unwrap_or(String::from("teamsLink")));
    let issue = Issue::get_issue(jira_api, &request.issue.id).await.context("Failed to get comment issue by id");
    let issue = match issue {
        Ok(i) => i,
        Err(e) => {
            println!("{:?}", e);
            return Err(e);
        }
    };
println!("2-4");
println!("{:#?}", issue);

    if let Some(message_id) = extract_message_id_from_url(issue.get_teams_link().unwrap_or_default()) {
println!("2-5");
        let reply_body = parse_markdown(request.comment.body.as_str());
println!("2-6");

        if let Some(reply_id) = request.comment.get_reply_id() {
println!("2-7");
            graph_api
                .edit_reply(&message_id, &reply_id, &reply_body)
                .await
                .context("Failed to update reply in channel")?;
println!("2-8");
        } else {
println!("2-9");
            let reply_id = graph_api
                .reply_to_issue(&message_id, &reply_body)
                .await
                .context("Failed to add reply to the channel")?
                .id;
println!("2-10");
            request.comment.add_reply_id(jira_api, &reply_id).await?;
println!("2-11");
        }
    }
println!("2-12");

    Ok(())
}

async fn parse_issue(payload: Bytes, graph_api: &MSGraphAPIShared) -> Result<()> {
    let request = serde_json::from_slice::<IssueRequest>(&payload)
        .context("Failed to deserialize payload")?;

    if let Some(link) = request.issue.get_teams_link() {
        if request
            .changelog
            .items
            .iter()
            .any(|i| i.field.to_lowercase() == String::from("status"))
        {
            if let Some(message_id) = extract_message_id_from_url(link.to_string()) {
                let reply_body = format!("Статус задачи изменён на {}", request.issue.get_status().unwrap_or_default());
                graph_api
                    .reply_to_issue(&message_id, &reply_body)
                    .await
                    .context("Failed to send notification to the channel")?;
            }
        }
    }

    Ok(())
}
