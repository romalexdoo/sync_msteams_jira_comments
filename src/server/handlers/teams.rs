use axum::{
    extract::{Query, State}, 
    http::StatusCode, 
    response::Result, 
    Json,
};
use serde::Deserialize;
use tracing::warn;

use crate::{
    jira_api::{comment::JiraComment, issue::Issue}, 
    ms_graph_api::message::MsGraphMessage, server::server::AppStateShared, 
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
    State(state_shared): State<AppStateShared>,
    req: Option<Json<Request>>, 
) -> Result<(StatusCode, String)> {
    log_to_file("teams", &format!("{:?}", req)).await;
    log_to_file("teams", &format!("{:?}", query.validation_token)).await;
    println!("Received request: {:?}", req);
    println!("Validation token: {:?}", query.validation_token);
    warn!("Received request: {:?}", req);
    warn!("Validation token: {:?}", query.validation_token);
    
    if let Some(Json(request)) = req {
        tokio::task::spawn(
            async move { 
                if let Err(e) = handle_teams_request(request, state_shared).await {
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

async fn handle_teams_request(request: Request, state_shared: AppStateShared) -> anyhow::Result<()> {
    let mut tx = state_shared.microsoft.state.lock().await;

    let token = match tx.token.get() {
        Ok(t) => t,
        Err(_) => {
            tx.token.renew(&state_shared.microsoft.client, &state_shared.microsoft.config).await?
        },
    };

    for value in request.value {
        tx.subscription.check_client_secret(&value.client_state)?;

        let (maybe_message_id, maybe_reply_id) = helpers::get_message_id_and_reply_id(&value.resource);
        
        if let Some(message_id) = maybe_message_id {
            let message = MsGraphMessage::get(&state_shared.microsoft.client, &value.resource, &token).await?;

            let user_email = match message.from.user {
                Some(u) => state_shared.microsoft.get_user_email(&token, u.id).await?,
                None => String::new(),
            };

            if user_email == state_shared.microsoft.config.teams_user {
                continue;
            }

            if let Some(reply_id) = maybe_reply_id {
                let message_url = &value.resource.split("/replies").next().unwrap_or_default().to_string();
                let parent_message = MsGraphMessage::get(&state_shared.microsoft.client, message_url, &token).await?;

                JiraComment::create_or_update(
                        state_shared.clone(),
                        &message.body.content, 
                        &user_email, 
                        &message.attachments,
                        &token,
                        &parent_message.web_url.unwrap_or_default(),
                        &reply_id,
                        &message_id,
                    )
                    .await?;
            } else {
                let (issue, issue_exists) = Issue::create_or_update(
                        state_shared.clone(),
                        &message.subject.unwrap_or_default(), 
                        &message.body.content, 
                        &user_email, 
                        &message.attachments,
                        &token,
                        &message.web_url.unwrap_or_default(),
                        &message_id,
                    )
                    .await?;

                if !issue_exists {
                    let url = format!("{}/browse/{}", state_shared.jira.config.base_url, issue.get_key());

                    state_shared.microsoft
                        .reply_to_issue(&message_id, &format!("<a href=\"{}\">{}</a>", url, url))
                        .await?;
                }
            }
        }
    }

    Ok(())
}