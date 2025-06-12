use anyhow::Result;
use clap::Parser;
use nym_node_requests::api::v1::node::models::NodeDescription;
use nym_rpc::tcp_proxy_server::{TcpProxyHttpConfig, TcpProxyServer};
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber;

#[derive(Debug, Parser)]
#[command(name = "nym-rpc-server")]
#[command(about = "Run a Nym TCP proxy server")]
struct Args {
    /// Path to the configuration directory
    #[clap(short, long, default_value = ".")]
    config_dir: PathBuf,

    /// Environment file path
    #[clap(short, long)]
    env: Option<String>,

    /// HTTP API bind address
    #[clap(long, default_value = "0.0.0.0:8080")]
    http_bind_address: String,

    /// Node description name/moniker
    #[clap(long, default_value = "Nym TCP Proxy Server")]
    description_name: String,

    /// Node description details
    #[clap(long, default_value = "Anonymous TCP proxy using Nym mixnet")]
    description_text: String,

    /// Node website/link (optional)
    #[clap(long)]
    description_link: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging FIRST, before anything else
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();

    info!("Starting Nym TCP Proxy Server...");

    let config_dir = args.config_dir.to_string_lossy().to_string();

    info!("HTTP API enabled on {}", args.http_bind_address);

    let description = NodeDescription {
        moniker: args.description_name,
        details: args.description_text,
        website: args.description_link.unwrap_or_default(),
        security_contact: "".to_string(), // Could be made configurable
    };

    let http_config = TcpProxyHttpConfig {
        bind_address: args.http_bind_address.parse()?,
        description,
        expose_system_info: true,
    };

    let mut server = TcpProxyServer::new_with_http(&config_dir, args.env, http_config).await?;

    // Start HTTP server if enabled
    let _http_handle = server.start_http_server().await?;

    info!("✓ Server initialized successfully");
    info!("Nym address: {}", server.nym_address());

    info!("HTTP API available at: http://{}", args.http_bind_address);
    info!("Try: http://{}/api/v1/description", args.http_bind_address);
    info!("Try: http://{}/api/v1/health", args.http_bind_address);

    info!("Starting TCP proxy server...");

    // Run the server
    if let Err(e) = server.run_with_shutdown().await {
        error!("Server error: {}", e);
        return Err(e);
    }

    info!("Server shut down gracefully");
    Ok(())
}
