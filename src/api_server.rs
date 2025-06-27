//! HTTP API server for Nym TCP Proxy Server
//! provides NYM-like API endpoints for nym-rpc-server
//! This can be expanded to provide more endpoints if needed

use anyhow::Result;
use axum::{Json, Router, extract::State, response::Html};
use nym_node_requests::api::v1::health::models::NodeHealth;
use nym_node_requests::api::v1::node::models::NodeDescription;
use nym_sdk::mixnet::Recipient;
use serde_json::json;
use std::net::SocketAddr;
use std::time::Instant;
use tracing::info;

// configuration for our TCP proxy with HTTP API
#[derive(Debug, Clone)]
pub struct TcpProxyHttpConfig {
    pub bind_address: SocketAddr,
    pub description: NodeDescription,
    pub expose_system_info: bool,
}

impl Default for TcpProxyHttpConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0:8080".parse().unwrap(),
            description: NodeDescription::default(),
            expose_system_info: true,
        }
    }
}

// Application state for HTTP endpoints
#[derive(Clone)]
pub struct TcpProxyAppState {
    pub description: NodeDescription,
    pub nym_address: Recipient,
    pub startup_time: Instant,
}

/// Build HTTP server with Nym-compatible endpoints
pub async fn build_http_server(
    nym_address: &Recipient,
    startup_time: Instant,
    http_config: &TcpProxyHttpConfig,
) -> Result<Router> {
    let state = TcpProxyAppState {
        description: http_config.description.clone(),
        nym_address: *nym_address,
        startup_time,
    };

    let router = Router::new()
        .route("/api/v1/health", axum::routing::get(tcp_proxy_health))
        .route(
            "/api/v1/description",
            axum::routing::get(tcp_proxy_description),
        )
        .route(
            "/api/v1/nym-address",
            axum::routing::get(tcp_proxy_nym_address),
        )
        .route("/", axum::routing::get(landing_page))
        .with_state(state);

    Ok(router)
}

/// Start HTTP server
pub async fn start_http_server(
    nym_address: &Recipient,
    startup_time: Instant,
    http_config: &TcpProxyHttpConfig,
) -> Result<tokio::task::JoinHandle<Result<()>>> {
    let router = build_http_server(nym_address, startup_time, http_config).await?;
    let bind_address = http_config.bind_address;

    info!(
        "Starting Nym-compatible HTTP API server on {}",
        bind_address
    );

    let handle = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(bind_address)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to bind HTTP server: {}", e))?;

        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("HTTP server error: {}", e))?;

        Ok(())
    });

    Ok(handle)
}

// HTTP API Handlers using Nym's components
/// Returns the description of this TCP proxy node - compatible with Nym API
pub async fn tcp_proxy_description(State(state): State<TcpProxyAppState>) -> Json<NodeDescription> {
    Json(state.description)
}

/// Returns health status - compatible with Nym health API  
pub async fn tcp_proxy_health(State(state): State<TcpProxyAppState>) -> Json<NodeHealth> {
    let uptime = state.startup_time.elapsed();
    let health = NodeHealth::new_healthy(uptime);
    Json(health)
}

/// Returns the Nym address of this TCP proxy
pub async fn tcp_proxy_nym_address(
    State(state): State<TcpProxyAppState>,
) -> Json<serde_json::Value> {
    Json(json!({
        "nym_address": state.nym_address.to_string(),
        "service": "nym-rpc-server"
    }))
}

/// Landing page
pub async fn landing_page() -> Html<&'static str> {
    Html(
        r#"
        <h1>Nym TCP Proxy Server</h1>
        <div>
            <p>This is a Nym TCP Proxy Server providing secure, anonymous TCP routing through the Nym mixnet.</p>
            <h2>API Endpoints:</h2>
            <ul>
                <li><a href="/api/v1/description">/api/v1/description</a> - Node description (Nym-compatible)</li>
                <li><a href="/api/v1/health">/api/v1/health</a> - Health check (Nym-compatible)</li>
                <li><a href="/api/v1/nym-address">/api/v1/nym-address</a> - Nym mixnet address</li>
            </ul>
            <p>This server reuses Nym's HTTP API components for maximum compatibility.</p>
        </div>
        "#,
    )
}
