//! HTTP API server for Nym TCP Proxy Server
//! Provides Nym-compatible API endpoints for node information, health checks, and session management
//! This is an overkill right now

use anyhow::Result;
use axum::{Json, Router, extract::State, response::Html};
use dashmap::DashMap;
use nym_bin_common::bin_info_owned;
use nym_crypto::asymmetric::{ed25519, x25519};
use nym_node_requests::api::v1::health::models::NodeHealth;
use nym_node_requests::api::v1::node::models::{
    AuxiliaryDetails, BinaryBuildInformationOwned, HostInformation, HostKeys, NodeDescription,
    NodeRoles,
};
use nym_sdk::mixnet::Recipient;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;
use uuid::Uuid;

// Configuration for our TCP proxy with HTTP API
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
    pub sessions: Arc<DashMap<Uuid, String>>,
    // Add required fields for described nodes
    pub roles: NodeRoles,
    pub host_information: HostInformation,
    pub build_information: BinaryBuildInformationOwned,
    pub auxiliary_details: AuxiliaryDetails,
}

/// Build HTTP server with Nym-compatible endpoints
pub async fn build_http_server(
    nym_address: &Recipient,
    sessions: Arc<DashMap<Uuid, String>>,
    startup_time: Instant,
    http_config: &TcpProxyHttpConfig,
) -> Result<Router> {
    // Create proper host information with your node's keys
    // Extract the actual identity and encryption keys from the Nym address
    let identity_key_str = nym_address.identity().to_base58_string();
    let encryption_key_str = nym_address.encryption_key().to_base58_string();

    let keys = HostKeys {
        ed25519_identity: ed25519::PublicKey::from_base58_string(&identity_key_str).unwrap(),
        x25519_sphinx: x25519::PublicKey::from_base58_string(&encryption_key_str).unwrap(),
        x25519_noise: None, // Optional
    };

    let host_information = HostInformation {
        ip_address: vec!["95.216.196.110".parse().unwrap()], // Replace with your actual IP
        hostname: None,
        keys,
    };

    // Define what services your TCP proxy provides
    let roles = NodeRoles {
        mixnode_enabled: false,
        gateway_enabled: false,
        network_requester_enabled: false,
        ip_packet_router_enabled: false,
    };

    let auxiliary_details = AuxiliaryDetails {
        location: None, // Add your country code if desired, e.g. Some(Country::from_alpha2("US").unwrap())
        announce_ports: Default::default(),
        accepted_operator_terms_and_conditions: true,
    };

    let state = TcpProxyAppState {
        description: http_config.description.clone(),
        nym_address: *nym_address,
        startup_time,
        sessions,
        roles,
        host_information,
        build_information: bin_info_owned!(),
        auxiliary_details,
    };

    // Build router with ALL required endpoints for described nodes
    let router = Router::new()
        // REQUIRED endpoints for described nodes
        .route("/api/v1/health", axum::routing::get(tcp_proxy_health))
        .route(
            "/api/v1/description",
            axum::routing::get(tcp_proxy_description),
        )
        .route(
            "/api/v1/host-information",
            axum::routing::get(tcp_proxy_host_information),
        )
        .route("/api/v1/roles", axum::routing::get(tcp_proxy_roles))
        // Optional but recommended
        .route(
            "/api/v1/build-information",
            axum::routing::get(tcp_proxy_build_info),
        )
        .route(
            "/api/v1/auxiliary-details",
            axum::routing::get(tcp_proxy_auxiliary_details),
        )
        // Your custom endpoints
        .route(
            "/api/v1/nym-address",
            axum::routing::get(tcp_proxy_nym_address),
        )
        .route("/api/v1/sessions", axum::routing::get(tcp_proxy_sessions))
        .route("/", axum::routing::get(landing_page))
        .with_state(state);

    Ok(router)
}

/// Start HTTP server
pub async fn start_http_server(
    nym_address: &Recipient,
    sessions: Arc<DashMap<Uuid, String>>,
    startup_time: Instant,
    http_config: &TcpProxyHttpConfig,
) -> Result<tokio::task::JoinHandle<Result<()>>> {
    let router = build_http_server(nym_address, sessions, startup_time, http_config).await?;
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

/// Returns information about active sessions
pub async fn tcp_proxy_sessions(State(state): State<TcpProxyAppState>) -> Json<serde_json::Value> {
    let session_count = state.sessions.len();
    Json(json!({
        "active_sessions": session_count,
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
                <li><a href="/api/v1/sessions">/api/v1/sessions</a> - Session information</li>
            </ul>
            <p>This server reuses Nym's HTTP API components for maximum compatibility.</p>
        </div>
        "#,
    )
}

/// Returns host information - REQUIRED for described nodes
pub async fn tcp_proxy_host_information(
    State(state): State<TcpProxyAppState>,
) -> Json<HostInformation> {
    Json(state.host_information)
}

/// Returns node roles - REQUIRED for described nodes  
pub async fn tcp_proxy_roles(State(state): State<TcpProxyAppState>) -> Json<NodeRoles> {
    Json(state.roles)
}

/// Returns build information
pub async fn tcp_proxy_build_info(
    State(state): State<TcpProxyAppState>,
) -> Json<BinaryBuildInformationOwned> {
    Json(state.build_information)
}

/// Returns auxiliary details
pub async fn tcp_proxy_auxiliary_details(
    State(state): State<TcpProxyAppState>,
) -> Json<AuxiliaryDetails> {
    Json(state.auxiliary_details)
}
