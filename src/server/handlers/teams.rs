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
    server::error::{Context, Error}
};

use super::helpers;


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
    if let Some(Json(request)) = req {

        let mut tx = graph_api.state.lock().await;

        let token = match tx.token.get() {
            Ok(t) => t,
            Err(_) => tx.token.renew(&graph_api.client, &graph_api.config).await.map_err(Error::c500).context("Failed to get token")?,
        };

        for value in request.value {
            tx.subscription.check_client_secret(&value.client_state).map_err(Error::c500).context("Failed to check secret")?;
            let (message_id, reply_id) = helpers::get_message_id_and_reply_id(&value.resource);
            
            if message_id.is_some() {
                let message = MsGraphMessage::get(&graph_api.client, &value.resource, &token).await.map_err(Error::c500).context("Failed to get Message")?;

                let user_email = match message.from.user {
                    Some(u) => graph_api.get_user_email(&token, u.id).await.map_err(Error::c500).context("Failed to get Teams user email")?,
                    None => String::new(),
                };

                if user_email == graph_api.config.teams_user {
                    continue;
                }

                if let Some(reply_id) = reply_id {
                    let message_url = &value.resource.split("/replies").next().unwrap_or_default().to_string();
                    let parent_message = MsGraphMessage::get(&graph_api.client, message_url, &token).await.map_err(Error::c500).context("Failed to get Message")?;

                    let result = JiraComment::create_or_update(
                            &jira_api,
                            &message.body.content, 
                            &user_email, 
                            &message.attachments,
                            &token,
                            &parent_message.web_url.unwrap_or_default(),
                            &reply_id,
                            &graph_api,
                            &message_id.unwrap(),
                        )
                        .await
                        .map_err(Error::c500)
                        .context("Failed to create comment in Jira");
                    if let Err(e) = result {
                        println!("{:?}", e.to_string());
                        return Err(e.into());
                    }
                } else {
                    let message_id_unwrapped = message_id.unwrap();
                    
                    let (issue, issue_exists) = Issue::create_or_update(
                            &jira_api,
                            &message.subject.unwrap_or_default(), 
                            &message.body.content, 
                            &user_email, 
                            &message.attachments,
                            &token,
                            &message.web_url.unwrap_or_default(),
                            &graph_api,
                            &message_id_unwrapped,
                        )
                        .await
                        .map_err(Error::c500)
                        .context("Failed to create Issue in Jira")?;

                    if !issue_exists {
                        let url = format!("{}/browse/{}", jira_api.config.base_url, issue.get_key());

                        graph_api
                            .reply_to_issue(&message_id_unwrapped, &format!("<a href=\"{}\">{}</a>", url, url))
                            .await
                            .map_err(Error::c500)
                            .context("Failed to send reply for teams topic")?;
                    }
                }
            }
        }
    };
    
    let response = match query {
        Some(Query(q)) => q.validation_token,
        None => String::new(),
    };

    Ok((StatusCode::OK, response))
}