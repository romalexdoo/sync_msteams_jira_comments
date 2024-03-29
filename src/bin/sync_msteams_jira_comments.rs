use sync_msteams_jira_comments::{
    cfg::Config, jira_api::model::JiraAPI, ms_graph_api::model::MSGraphAPI, server::server::Server, utils::os_signal_or_completion_of
};

use anyhow::{ Context, Result };
use envconfig::Envconfig;
use std::{sync::Arc, time::Duration};


#[tokio::main]
async fn main() -> Result<()> {
    // Tracing.
    tracing_subscriber::fmt().with_max_level(tracing::Level::WARN).init();
    // Read configuration.
    let cfg = Config::init_from_env().context("parse config")?;
    // Create MSGraphAPI instance
    let graph_api = Arc::new(MSGraphAPI::new(cfg.ms_graph_api.clone())?);
    // Create JiraAPI instance
    let jira_api = Arc::new(JiraAPI::new(cfg.jira.clone())?);
    // Create API server.
    let api_server = Server::new();
    // Start API server, but do not call await.
    let api_server_future = api_server.start(Arc::new(cfg.clone()), graph_api.clone(), jira_api.clone());
    // Wait server start and init subscription
    let api = graph_api.clone();
    tokio::task::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let mut tx = api.state.lock().await;
        let token = match tx.token.get() {
            Ok(t) => t,
            Err(_) => tx.token.renew(&api.client, &api.config).await.unwrap(),
        };

        tx.subscription.init(&api.client, &api.config, &token, false).await.unwrap();
    });
    // Renew delegated access token when needed
    let api = graph_api.clone();
    tokio::task::spawn(async move {
        api.manage_granted_token().await.unwrap();
    });
    // Block until termination signal is received from OS or API server fails.
    let api_server_result = os_signal_or_completion_of(api_server_future).await;
    // Gracefully stop API server if not already stopped.
    api_server.stop(Duration::from_secs(cfg.server.shutdown_timeout));
    // Return result.
    api_server_result.context("API server")
}