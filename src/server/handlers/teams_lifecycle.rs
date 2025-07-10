use anyhow::Context as _;
use axum::{
    body::Bytes, extract::{Query, State}, http::StatusCode, response::{
        IntoResponse, 
        Result
    },
};
use serde::Deserialize;
use tracing::error;

use crate::{
    ms_graph_api::model::MSGraphAPI, server::{error::Error, server::AppStateShared}
};

use super::helpers;


#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Request {
    pub(crate) value: Option<Vec<RequestValue>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RequestValue {
    pub(crate) lifecycle_event: String,
    pub(crate) client_state: String,
    pub(crate) subscription_id: String,
}


pub(crate) async fn handler(
    Query(query): Query<helpers::ValidationTokenQuery>, 
    State(state_shared): State<AppStateShared>,
    body: Bytes,
) -> Result<impl IntoResponse> {
    let mut reply_status = StatusCode::ACCEPTED;

    if !body.is_empty() {
        let request = serde_json::from_slice::<Request>(&body).map_err(|e| {
            error!("Failed to parse request body: {}", e);
            (StatusCode::BAD_REQUEST, e.to_string())
        })?;
        
        if let Err(e) = parse_handler(&state_shared.microsoft, request).await {
            error!("Failed to handle teams lifecycle request: {}", e);
            return Err(Error::c500(e).into());
        }
    };

    let response = match query.validation_token {
        Some(ref q) => {
            reply_status = StatusCode::OK;
            q.clone()
        },
        None => String::new(),
    };

    Ok((reply_status, response))
}

async fn parse_handler(graph_api: &MSGraphAPI, request: Request) -> anyhow::Result<()> {
    let mut tx = graph_api.state.lock().await;
        
    let token = tx.token.renew(&graph_api.client, &graph_api.config).await.context("Failed to get token")?;

    if let Some(values) = request.value {
        for value in values {
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
    }

    Ok(())
}
