use axum::{
    extract::{Form, State}, 
    response::Html,
};
use serde::Deserialize;

use crate::ms_graph_api::model::MSGraphAPIShared;


#[derive(Debug, Deserialize)]
pub(crate) struct OAuthRequest {
    pub(crate) code: String,
    pub(crate) state: String,
}

pub(crate) async fn handler(
    State(graph_api): State<MSGraphAPIShared>,
    Form(data): Form<OAuthRequest>,
) -> Html<String> {
    
    if graph_api.state.lock().await.subscription.check_client_secret(&data.state).is_err() {
        return get_html("Error", "Failed to check secret");
    }

    if graph_api.set_delegated_token(data.code).await.is_err() {
        return get_html("Error", "Failed to set delegated token");
    }

    get_html("Authentication successful", "Authentication successful! Please close this tab.")
}

fn get_html(title: &str, body: &str) -> Html<String> {
    let template = r#"
                            <!DOCTYPE html>
                            <html lang="en">
                            <head>
                                <meta charset="UTF-8">
                                <title>"{TITLE}</title>
                            </head>
                            <body>
                                <p>{BODY}</p>
                            </body>
                            </html>    
                        "#;
    Html(template.replace("{TITLE}", title).replace("{BODY}", body))
}