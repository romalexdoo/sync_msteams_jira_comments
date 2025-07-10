use crate::cfg::Config;
use crate::jira_api::model::JiraAPI;
use crate::server::handlers::{jira, teams, teams_lifecycle, ms_oauth};
use crate::ms_graph_api::model::MSGraphAPI;
use anyhow::{ Context, Result };
use axum::body::{to_bytes, Body};
use axum::extract::Request;
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::http::HeaderValue;
use axum::{
    Router,
    routing::post,
};
use axum_server::Handle;
use tracing::info;
use std::sync::Arc;
use std::time::Duration;
use tower_http::compression::CompressionLayer;

/// API server with some default middleware and endpoints.
#[derive(Clone)]
pub struct Server {
    handle: Handle,
}

pub struct AppState {
    pub jira: JiraAPI,
    pub microsoft: MSGraphAPI,
}

pub type AppStateShared = Arc<AppState>;

impl Server {
    pub fn new() -> Self {
        Self { handle: Handle::new() }
    }

    /// Starts API server.
    pub async fn start(&self, cfg: Config, state_shared: AppStateShared) -> Result<()> {
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
            .layer(middleware::from_fn(add_host_header_middleware))
            .layer(middleware::from_fn(log_request_middleware))
            // Injects MS Graph API.
            .with_state(state_shared)
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

pub async fn add_host_header_middleware(
    mut req: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    // Check if Host header is missing and add a default one
    if !req.headers().contains_key("host") {
        req.headers_mut().insert(
            "host", 
            HeaderValue::from_static("microsoft server")
        );
        info!("Added default Host header for request without one");
    }
    
    next.run(req).await
}

pub async fn log_request_middleware(
    req: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    // Extract and clone the body
    let (parts, body) = req.into_parts();
    
    // Extract query string from parts
    let query = parts.uri.query().unwrap_or_default();
    let whole_body = to_bytes(body, usize::MAX).await.unwrap_or_default();
    let body_str = String::from_utf8_lossy(&whole_body);

    // Log headers for debugging
    let headers: Vec<String> = parts.headers.iter()
        .map(|(name, value)| format!("{}: {}", name, value.to_str().unwrap_or("<invalid>")))
        .collect();

    // Log the request details
    info!("Incoming request query: {}", query);
    info!("Incoming request headers: {}", headers.join(", "));
    info!("Incoming request body: {}", body_str);

    // Replace original body with cloned body for the next handler
    let req = Request::from_parts(parts, Body::from(whole_body));

    // Proceed to the next middleware/handler
    next.run(req).await
}
