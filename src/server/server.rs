use crate::cfg::Config;
use crate::jira_api::model::JiraAPIShared;
use crate::server::handlers::{jira, teams, teams_lifecycle, ms_oauth};
use crate::ms_graph_api::model::MSGraphAPIShared;
use anyhow::{ Context, Result };
use axum::{
    Extension,
    Router,
    routing::post,
};
use axum_server::Handle;
use std::sync::Arc;
use std::time::Duration;
use tower_http::compression::CompressionLayer;

/// API server with some default middleware and endpoints.
#[derive(Clone)]
pub struct Server {
    handle: Handle,
}

impl Server {
    pub fn new() -> Self {
        Self { handle: Handle::new() }
    }

    /// Starts API server.
    pub async fn start(&self, cfg: Arc<Config>, graph_api: MSGraphAPIShared, jira_api: JiraAPIShared) -> Result<()> {
        // Create router.
        // Middleware ordering matters!
        // Request processing starts from last layer.
        // Response processing starts from first layer.
        let router = Router::new()
            // API router.
            .route("/jira", post(jira::handler))
            .route("/teams", post(teams::handler))
            .route("/teams_lifecycle", post(teams_lifecycle::handler))
            .route("/ms_oauth", post(ms_oauth::handler))
            // Injects Jira API.
            .layer(Extension(jira_api))
            // Injects MS Graph API.
            .with_state(graph_api)
            // Compression.
            .layer(CompressionLayer::new());
        
        // Start API server.
        axum_server::bind(cfg.server.addr.parse()?)
            .handle(self.handle.clone())
            .serve(router.into_make_service())
            .await
            .context("API server")
    }

    /// Gracefully stops API server.
    pub fn stop(&self, timeout: Duration) {
        self.handle.graceful_shutdown(Some(timeout));
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}
