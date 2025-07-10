use axum::{
    extract::{Query, State}, 
    http::StatusCode, 
    response::Result, 
    Extension, 
    Json,
};
use serde::Deserialize;
use tracing::warn;

use crate::{
    jira_api::{comment::JiraComment, issue::Issue, model::JiraAPIShared}, 
    ms_graph_api::{message::MsGraphMessage, model::MSGraphAPIShared}, 
};

use super::helpers::{self, log_to_file};


#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Request {
    pub(crate) value: Vec<RequestValue>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestValue {
    // pub(crate) change_type: String,
    pub(crate) client_state: String,
    pub(crate) resource: String,
}

pub(crate) async fn handler(
    Query(query): Query<helpers::ValidationTokenQuery>, 
    Extension(jira_api): Extension<JiraAPIShared>,
    State(graph_api): State<MSGraphAPIShared>,
    req: Option<Json<Request>>, 
) -> Result<(StatusCode, String)> {
    warn!("Received request: {:?}", req);
    warn!("Query: {:?}", query.validation_token);
    
    if let Some(Json(request)) = req {
        tokio::task::spawn(
            async move { 
                if let Err(e) = handle_teams_request(request, &graph_api, &jira_api).await {
                    log_to_file("teams", &e.to_string()).await;
                    Err(e)
                } else {
                    Ok(())
                }
            }
        );
    };
    
    let response = query.validation_token.clone().unwrap_or_default();

    Ok((StatusCode::OK, response))
}

async fn handle_teams_request(request: Request, graph_api: &MSGraphAPIShared, jira_api: &JiraAPIShared) -> anyhow::Result<()> {
    let mut tx = graph_api.state.lock().await;

    let token = match tx.token.get() {
        Ok(t) => t,
        Err(_) => {
            tx.token.renew(&graph_api.client, &graph_api.config).await?
        },
    };

    for value in request.value {
        tx.subscription.check_client_secret(&value.client_state)?;

        let (maybe_message_id, maybe_reply_id) = helpers::get_message_id_and_reply_id(&value.resource);
        
        if let Some(message_id) = maybe_message_id {
            let message = MsGraphMessage::get(&graph_api.client, &value.resource, &token).await?;

            let user_email = match message.from.user {
                Some(u) => graph_api.get_user_email(&token, u.id).await?,
                None => String::new(),
            };

            if user_email == graph_api.config.teams_user {
                continue;
            }

            if let Some(reply_id) = maybe_reply_id {
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
                let (issue, issue_exists) = Issue::create_or_update(
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
                    .await?;

                if !issue_exists {
                    let url = format!("{}/browse/{}", jira_api.config.base_url, issue.get_key());

                    graph_api
                        .reply_to_issue(&message_id, &format!("<a href=\"{}\">{}</a>", url, url))
                        .await?;
                }
            }
        }
    }

    Ok(())
}