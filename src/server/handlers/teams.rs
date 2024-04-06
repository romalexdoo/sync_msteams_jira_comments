use axum::{
    extract::{Query, State}, 
    http::StatusCode, 
    response::Result, 
    Extension, 
    Json,
};
use serde::Deserialize;

use crate::{
    jira_api::{comment::JiraComment, issue::Issue, model::JiraAPIShared}, 
    ms_graph_api::{message::MsGraphMessage, model::MSGraphAPIShared}, 
};

use super::helpers::{self, log_to_file};


#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub value: Vec<RequestValue>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestValue {
    pub change_type: String,
    pub client_state: String,
    pub resource: String,
}

pub async fn handler(
    query: Option<Query<helpers::ValidationTokenQuery>>, 
    Extension(jira_api): Extension<JiraAPIShared>,
    State(graph_api): State<MSGraphAPIShared>,
    req: Option<Json<Request>>, 
) -> Result<(StatusCode, String)> {
    log_to_file("Teams", "Start").await;
    
    if let Some(Json(request)) = req {
        tokio::task::spawn(async move { handle_teams_request(request, &graph_api, &jira_api).await });
    };
    
    let response = match query {
        Some(Query(q)) => q.validation_token,
        None => String::new(),
    };

    Ok((StatusCode::OK, response))
}

async fn handle_teams_request(request: Request, graph_api: &MSGraphAPIShared, jira_api: &JiraAPIShared) -> anyhow::Result<()> {
    log_to_file("Teams", format!("Received JSON {:?}", request).as_str()).await;

    let mut tx = graph_api.state.lock().await;
    log_to_file("Teams", "Got mutex").await;

    let token = match tx.token.get() {
        Ok(t) => t,
        Err(_) => {
            log_to_file("Teams", "Try new token").await;
            tx.token.renew(&graph_api.client, &graph_api.config).await?
        },
    };
    log_to_file("Teams", "Got token").await;

    for value in request.value {
        log_to_file("Teams", format!("Request value {:?}", value).as_str()).await;

        tx.subscription.check_client_secret(&value.client_state)?;
        log_to_file("Teams", "Subscription secret OK").await;
        let (maybe_message_id, maybe_reply_id) = helpers::get_message_id_and_reply_id(&value.resource);
        
        if let Some(message_id) = maybe_message_id {
            log_to_file("Teams", "Retrieved Message ID").await;
            let message = MsGraphMessage::get(&graph_api.client, &value.resource, &token).await?;
            log_to_file("Teams", format!("Got MSGraph message {:?}", message).as_str()).await;

            let user_email = match message.from.user {
                Some(u) => graph_api.get_user_email(&token, u.id).await?,
                None => String::new(),
            };

            log_to_file("Teams", format!("Got MSGraph user email {}", user_email).as_str()).await;

            if user_email == graph_api.config.teams_user {
                continue;
            }

            if let Some(reply_id) = maybe_reply_id {
                log_to_file("Teams", "Has reply ID, not interesting").await;

                let message_url = &value.resource.split("/replies").next().unwrap_or_default().to_string();
                let parent_message = MsGraphMessage::get(&graph_api.client, message_url, &token).await?;

                JiraComment::create_or_update(
                        &jira_api,
                        &message.body.content, 
                        &user_email, 
                        &message.attachments,
                        &token,
                        &parent_message.web_url.unwrap_or_default(),
                        &reply_id,
                        &graph_api,
                        &message_id,
                    )
                    .await?;
            } else {
                log_to_file("Teams", "Try to update/create issue").await;

                let (issue, issue_exists) = match Issue::create_or_update(
                        &jira_api,
                        &message.subject.unwrap_or_default(), 
                        &message.body.content, 
                        &user_email, 
                        &message.attachments,
                        &token,
                        &message.web_url.unwrap_or_default(),
                        &graph_api,
                        &message_id,
                    )
                    .await {
                        Ok((i, e)) => (i, e), 
                        Err(e) => {
                                log_to_file("Create or update issue in Jira", e.to_string().as_str()).await;
                                return Err(e);
                            },
                    };

                log_to_file(format!("Log issue (exists: {})", issue_exists).as_str(), issue.get_key().as_str()).await;

                if !issue_exists {
                    log_to_file("Teams", "Try to respond to MS Teams").await;
                    
                    let url = format!("{}/browse/{}", jira_api.config.base_url, issue.get_key());

                    match graph_api
                        .reply_to_issue(&message_id, &format!("<a href=\"{}\">{}</a>", url, url))
                        .await {
                            Ok(_) => (),
                            Err(e) => {
                                    log_to_file("Reply with issue ID to MS Teams", e.to_string().as_str()).await;
                                    return Err(e);
                                }
                        }

                    log_to_file("Teams", "Sent response to MS Teams").await;
                }
            }
        }
    }

    Ok(())
}