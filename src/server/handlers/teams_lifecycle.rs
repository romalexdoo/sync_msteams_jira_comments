use anyhow::Context as _;
use axum::{
    extract::{Query, State}, http::StatusCode, response::{
        IntoResponse, 
        Result
    }, Json
};
use serde::Deserialize;

use crate::{
    server::error::Error, 
    ms_graph_api::model::MSGraphAPIShared
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
    pub(crate) lifecycle_event: String,
    pub(crate) client_state: String,
    pub(crate) subscription_id: String,
}


pub(crate) async fn handler(
    query: Option<Query<helpers::ValidationTokenQuery>>, 
    State(graph_api): State<MSGraphAPIShared>,
    req: Option<Json<Request>>
) -> Result<impl IntoResponse> {
    let mut reply_status = StatusCode::ACCEPTED;

    if let Some(Json(request)) = req {
        if let Err(e) = parse_handler(graph_api, request).await {
            log_to_file("teams_lifecycle", &e.to_string()).await;
            return Err(Error::c500(e).into());
        }
    };

    let response = match query {
        Some(Query(q)) => {
            reply_status = StatusCode::OK;
            q.validation_token
        },
        None => String::new(),
    };

    Ok((reply_status, response))
}

async fn parse_handler(graph_api: MSGraphAPIShared, request: Request) -> anyhow::Result<()> {
    let mut tx = graph_api.state.lock().await;
        
    let token = tx.token.renew(&graph_api.client, &graph_api.config).await.context("Failed to get token")?;

    for value in request.value {
        tx.subscription.check_client_secret(&value.client_state).context("Failed to check secret")?;

        match value.lifecycle_event.as_str() {
            "reauthorizationRequired" => {
                    tx.subscription
                        .renew(&graph_api.client, &token, &value.subscription_id)
                        .await
                        .context("Failed to renew subscription")?;
                },
            "subscriptionRemoved" => {
                    tx.subscription
                        .init(&graph_api.client, &graph_api.config, &token, false)
                        .await
                        .context("Failed to init new subscription")?;
                },
            _ => (),
        }
    }

    Ok(())
}
