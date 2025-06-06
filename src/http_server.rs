//! HTTP server that forwards requests to the upstream server via the TCP proxy.
//! - Creates a HTTP client with custom DNS resolution to proxy
//! - Handles CORS preflight requests
//! - Handles response from the upstream server and sends it back to the client

use crate::common::RpcProviderUrl;
use anyhow::Result;
use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::{Method, StatusCode},
    response::Response,
    routing::any,
};
use reqwest::Client;
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use tokio::{signal, time::Instant};
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tracing::{error, info};
use url::Url;

/// Configuration for the MetaMask proxy server
#[derive(Clone)]
pub struct ProxyConfig {
    /// Address to listen on (typically 127.0.0.1)
    pub listen_address: String,
    /// Port to listen on (typically 8545 for RPC)
    pub listen_port: u16,
    /// TPC proxy URL to forward requests to
    pub tcp_proxy_url: String,
    /// Request timeout duration
    pub timeout: Duration,
    /// RPC provider URL
    pub rpc_provider_url: RpcProviderUrl,
}

/// Shared state for the proxy server
#[derive(Clone)]
struct AppState {
    client: Client,
    config: ProxyConfig,
}

/// Start the proxy server
pub async fn start_proxy_server(config: ProxyConfig) -> Result<()> {
    info!("🚀 Starting RPC Proxy Server");
    info!(
        "📍 Listening on: http://{}:{}",
        config.listen_address, config.listen_port
    );
    info!("🔗 Forwarding to NYM TCP proxy: {}", config.tcp_proxy_url);

    // Parse the TCP proxy URL to get the proxy address
    let proxy_url = Url::parse(&format!("http://{}", config.tcp_proxy_url))?;
    let proxy_host = proxy_url.host_str().unwrap_or("127.0.0.1");
    let proxy_port = proxy_url.port().unwrap_or(8080);
    let proxy_addr = format!("{}:{}", proxy_host, proxy_port);

    let client = Client::builder()
        .timeout(config.timeout)
        // Create HTTP client with custom DNS resolution to proxy
        // This mimics curl's --resolve flag: resolve the real hostname to our proxy address
        .resolve_to_addrs(
            &config.rpc_provider_url.host,
            &[proxy_addr.parse::<SocketAddr>()?],
        )
        .build()?;

    let state = AppState {
        client,
        config: config.clone(),
    };

    // Create CORS layer for MetaMask compatibility
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any);

    // Build the router
    let app = Router::new()
        .route("/", any(handle_proxy_request))
        .route("/*path", any(handle_proxy_request))
        .layer(ServiceBuilder::new().layer(cors))
        .with_state(state);

    // Create socket address
    let addr = SocketAddr::from((config.listen_address.parse::<IpAddr>()?, config.listen_port));

    // Start the server
    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Run server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Handle all proxy requests
async fn handle_proxy_request(
    State(state): State<AppState>,
    method: Method,
    request: Request,
) -> Result<Response<Body>, StatusCode> {
    let start_time = Instant::now();
    let path = request.uri().path();

    info!(
        "[{}] {} {} → {}",
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ"),
        method,
        path,
        state.config.tcp_proxy_url
    );

    // Handle OPTIONS requests for CORS preflight
    if method == Method::OPTIONS {
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .body(Body::empty())
            .unwrap());
    }

    // Extract request body for POST requests
    let body_bytes = axum::body::to_bytes(request.into_body(), usize::MAX)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // Build upstream URL using the REAL hostname (not proxy)
    // This works because we've configured DNS resolution to point the real hostname to our proxy
    let upstream_url = format!(
        "{}://{}{}",
        state.config.rpc_provider_url.scheme,
        state.config.rpc_provider_url.full_host, // Use real hostname for SSL validation
        state.config.rpc_provider_url.path
    );

    // Create the upstream request
    // No need to manually set Host header since we're using the real hostname in the URL
    let mut upstream_request = state
        .client
        .request(method.clone(), &upstream_url)
        .timeout(state.config.timeout);

    // Add request body if present
    if !body_bytes.is_empty() {
        upstream_request = upstream_request
            .header("Content-Type", "application/json")
            .body(body_bytes);
    }

    // Execute the upstream request
    match upstream_request.send().await {
        Ok(upstream_response) => {
            let status = upstream_response.status();
            let elapsed = start_time.elapsed();

            info!(
                "[{}] Response: {} ({}ms)",
                chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                status.as_u16(),
                elapsed.as_millis()
            );

            // Convert response
            let mut response_builder = Response::builder().status(status);

            // Copy response headers (excluding hop-by-hop headers)
            for (name, value) in upstream_response.headers() {
                let header_name = name.as_str().to_lowercase();
                if !is_hop_by_hop_header(&header_name) {
                    response_builder = response_builder.header(name, value);
                }
            }

            // Get response body
            let response_bytes = upstream_response.bytes().await.map_err(|e| {
                error!("Failed to read upstream response body: {}", e);
                StatusCode::BAD_GATEWAY
            })?;

            Ok(response_builder.body(Body::from(response_bytes)).unwrap())
        }
        Err(e) => {
            error!(
                "[{}] Proxy error: {}",
                chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                e
            );

            let error_response = serde_json::json!({
              "jsonrpc": "2.0",
              "error": {
                "code": -4999,
                "message": "mixnet request failed"
              },
              "id": null
            });

            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header("Content-Type", "application/json")
                .body(Body::from(error_response.to_string()))
                .unwrap())
        }
    }
}

/// Check if a header is a hop-by-hop header that should not be forwarded
fn is_hop_by_hop_header(header_name: &str) -> bool {
    matches!(
        header_name,
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "transfer-encoding"
            | "upgrade"
    )
}

/// Handle graceful shutdown signals
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("🛑 Received Ctrl+C, shutting down proxy server...");
        }
        _ = terminate => {
            info!("🛑 Received terminate signal, shutting down proxy server...");
        }
    }
}
