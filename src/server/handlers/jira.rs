use anyhow::{ensure, Context, Result};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header::HeaderMap, HeaderName, StatusCode};
use axum::response::Result as ApiResult;
use axum::Extension;
use hmac::{Hmac, Mac};
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use sha2::Sha256;
type HmacSha256 = Hmac<Sha256>;

use crate::jira_api::comment::JiraComment;
use crate::jira_api::issue::Issue;
use crate::jira_api::model::JiraAPIShared;
use crate::ms_graph_api::model::MSGraphAPIShared;
use crate::server::error::{Context as ApiContext, Error as ApiError};

use super::helpers::log_to_file;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IssueRequest {
    pub(crate) issue: Issue,
    pub(crate) changelog: ChangeLog,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentRequest {
    pub(crate) comment: JiraComment,
    pub(crate) issue: IssueId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IssueId {
    pub(crate) id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChangeLog {
    pub(crate) items: Vec<ChangeLogItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChangeLogItem {
    pub(crate) field: String,
    // pub(crate) to_string: String,
}

#[derive(Debug)]
struct Signature {
    method: String,
    value: String,
}

pub(crate) async fn handler(
    Extension(jira_api): Extension<JiraAPIShared>,
    State(graph_api): State<MSGraphAPIShared>,
    headers: HeaderMap,
    payload: axum::body::Bytes,
)-> ApiResult<StatusCode, ApiError> {
    match parse_handler(jira_api, graph_api, headers, payload).await {
        Ok(()) => Ok(StatusCode::OK),
        Err(e) => {
            log_to_file("jira", &e.to_string()).await;
            return Err(ApiError::c500(e));
        }
    }
}

async fn parse_handler(
    jira_api: JiraAPIShared,
    graph_api: MSGraphAPIShared,
    headers: HeaderMap,
    payload: axum::body::Bytes,
) -> Result<()> {

    let signature = get_signature_from_headers(headers)
        .context("Failed to get signature")?;

    validate_signature(&payload, &jira_api.config.secret, &signature)
        .context("Failed to validate signature")?;

    let json_payload = serde_json::from_slice::<Value>(&payload)
        .context("Failed to deserialize payload")?;

    let webhook_event = format!("{}",
            &json_payload
                .get("webhookEvent")
                .and_then(|t| t.as_str())
                .unwrap_or_default()
                .to_string()
        );

    tokio::task::spawn(async move { 
        handle_jira_request(webhook_event, payload, &jira_api, &graph_api).await 
    });

    Ok(())
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
    let author = jira_api.find_user_by_id(&request.comment.update_author.account_id).await.context("Failed to get author")?;

    if let Some(author_email) = author.email_address {
        if author_email.to_lowercase() == jira_api.config.user.to_lowercase() {
            return Ok(());
        }
    }

    let issue = Issue::get_issue(jira_api, &request.issue.id).await.context("Failed to get comment issue by id")?;

    if let Some(message_id) = extract_message_id_from_url(issue.get_teams_link().unwrap_or_default()) {
        let mut text = request.comment.body;

        let re = Regex::new(r"\[~accountid:([^\]]+)\]").expect("Failed to compile regex");

        for cap in re.captures_iter(&text.clone()) {
            let account_id = &cap[1].to_string();
            if let Ok(user) = jira_api.find_user_by_id(account_id).await {
                if let Some(username) = user.display_name.or(user.email_address) {
                    let full_match = cap.get(0).unwrap().as_str();
                    text = text.replace(full_match, format!("{}", username.as_str()).as_str());
                }
            }
        }
        
        let reply_body = markdown_to_html_parser::parse_markdown(&text);
        let comment = JiraComment::get(jira_api, &issue.get_id(), &request.comment.id).await?;

        if let Some(reply_id) = comment.get_reply_id() {
            graph_api
                .edit_reply(&message_id, &reply_id, &reply_body)
                .await
                .context("Failed to update reply in channel")?;
        } else {
            let reply_id = graph_api
                .reply_to_issue(&message_id, &reply_body)
                .await
                .context("Failed to add reply to the channel")?
                .id;
            comment.add_reply_id(jira_api, &reply_id).await?;
        }
    }

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

async fn handle_jira_request(webhook_event: String, payload: Bytes, jira_api: &JiraAPIShared, graph_api: &MSGraphAPIShared) -> anyhow::Result<()> {
    let result = match webhook_event.as_str() {
        "comment_created" | "comment_updated" => { 
                parse_comment(payload, jira_api, graph_api).await.context("Failed to parse comment")
            },
        _ => { 
                parse_issue(payload, graph_api).await.context("Failed to parse issue")
            },
    };

    if let Err(e) = result {
        log_to_file("handle jira request", &e.to_string()).await;
        return Err(e);
    }

    Ok(())
}