use axum::{
    extract::{Form, State}, 
    response::Html,
};
use serde::Deserialize;

use crate::server::server::AppStateShared;


#[derive(Debug, Deserialize)]
pub(crate) struct OAuthRequest {
    pub(crate) code: String,
    pub(crate) state: String,
}

pub(crate) async fn handler(
    State(state_shared): State<AppStateShared>,
    Form(data): Form<OAuthRequest>,
) -> Html<String> {

    if state_shared.microsoft.state.lock().await.subscription.check_client_secret(&data.state).is_err() {
        return get_html("Error", "Failed to check secret");
    }

    if state_shared.microsoft.set_delegated_token(data.code).await.is_err() {
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