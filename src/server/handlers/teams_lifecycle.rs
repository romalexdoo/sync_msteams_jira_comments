use axum::{
    extract::{Query, State}, http::StatusCode, response::{
        IntoResponse, 
        Result
    }, Json
};
use serde::Deserialize;

use crate::{
    server::error::{Context, Error}, 
    ms_graph_api::model::MSGraphAPIShared
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
    pub lifecycle_event: String,
    pub client_state: String,
    pub subscription_id: String,
}


pub async fn handler(
    query: Option<Query<helpers::ValidationTokenQuery>>, 
    State(graph_api): State<MSGraphAPIShared>,
    req: Option<Json<Request>>
) -> Result<impl IntoResponse> {
    let mut reply_status = StatusCode::ACCEPTED;

    if let Some(Json(request)) = req {
        let mut tx = graph_api.state.lock().await;
        
        let token = tx.token.renew(&graph_api.client, &graph_api.config).await.map_err(Error::c500).context("Failed to get token")?;

        for value in request.value {
            tx.subscription.check_client_secret(&value.client_state).map_err(Error::c500).context("Failed to check secret")?;

            match value.lifecycle_event.as_str() {
                "reauthorizationRequired" => {
                        tx.subscription
                            .renew(&graph_api.client, &token, &value.subscription_id)
                            .await
                            .map_err(Error::c500)
                            .context("Failed to renew subscription")?;
                    },
                "subscriptionRemoved" => {
                        tx.subscription
                            .init(&graph_api.client, &graph_api.config, &token, false)
                            .await
                            .map_err(Error::c500)
                            .context("Failed to init new subscription")?;
                    },
                _ => (),
            }
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