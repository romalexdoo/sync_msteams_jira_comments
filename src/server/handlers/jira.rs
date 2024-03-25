use anyhow::{ensure, Context, Result};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header::HeaderMap, HeaderName, StatusCode};
use axum::response::Result as ApiResult;
use axum::Extension;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::Value;
use sha2::Sha256;
type HmacSha256 = Hmac<Sha256>;

use crate::jira_api::issue::Issue;
use crate::jira_api::model::JiraAPIShared;
use crate::ms_graph_api::model::MSGraphAPIShared;
use crate::server::error::{Context as ApiContext, Error as ApiError};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub issue: Issue,
    pub changelog: ChangeLog,
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
println!("{:#?}", json_payload);
    let request = serde_json::from_slice::<Request>(&payload)
        .map_err(ApiError::c500)
        .context("Failed to deserialize payload")?;

    let teams_link = &json_payload["issue"]["fields"]
        .get(jira_api.config.msteams_link_field_name.clone())
        .and_then(|t| t.as_str());

    if let Some(link) = teams_link {
        if request
            .changelog
            .items
            .iter()
            .any(|i| i.field.to_lowercase() == String::from("status"))
        {
            if let Some(message_id) = extract_message_id_from_url(link.to_string()) {
                let reply_body = format!("Статус задачи изменён на {}", request.issue.get_status());
                graph_api
                    .reply_to_issue(&message_id, &reply_body)
                    .await
                    .map_err(ApiError::c500)
                    .context("Failed to send notification to the channel")?;
            }
        }
    }

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
