use anyhow::Result;
use clap::Parser;
use nym_rpc::common::RpcProviderUrl;
use nym_rpc::http_server::{ProxyConfig, start_proxy_server};
use nym_rpc::tcp_proxy_client::NymProxyClient;
use nym_sdk::mixnet::Recipient;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "nym-rpc-client")]
#[command(about = "Send RPC requests via the NYM mixnet")]
struct Args {
    /// Listen address for HTTP proxy
    #[clap(long, default_value = "127.0.0.1")]
    listen_address: String,

    /// Listen port for HTTP proxy
    #[clap(long, default_value = "8545")]
    listen_port: u16,

    /// Listen port for TCP proxy
    #[clap(long, default_value = "8080")]
    tcp_proxy_port: u16,

    /// Send timeout in seconds
    #[clap(long, default_value_t = 120)]
    request_timeout: u64,

    /// Nym address of the NymProxyServer e.g. EjYsntVxxBJrcRugiX5VnbKMbg7gyBGSp9SLt7RgeVFV.EzRtVdHCHoP2ho3DJgKMisMQ3zHkqMtAFAW4pxsq7Y2a@Hs463Wh5LtWZU@NyAmt4trcCbNVsuUhry1wpEXpVnAAfn
    #[clap(short = 's', long)]
    exit_node_address: String,

    /// Optional env filepath - if none is supplied then the proxy defaults to using mainnet else just use a path to one of the supplied files in envs/ e.g. ./envs/sandbox.env
    #[clap(short, long)]
    env_path: Option<String>,

    /// How many clients to have running in reserve for quick access by incoming connections
    #[clap(long, default_value_t = 2)]
    client_pool_reserve: usize,

    /// Provider URL (e.g., https://mainnet.infura.io/v3/YOUR_PROJECT_ID)
    #[clap(long)]
    rpc_provider: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    nym_bin_common::logging::setup_tracing_logger();
    let args = Args::parse();

    let exit_node_address: Recipient =
        Recipient::try_from_base58_string(&args.exit_node_address).expect("Invalid server address");

    // Extract HOST from RPC provider URL
    let rpc_provider_url = RpcProviderUrl::new(&args.rpc_provider);

    // Create TCP proxy client
    let proxy_client = NymProxyClient::new(
        exit_node_address,
        rpc_provider_url.full_host.clone(),
        &args.listen_address,
        &args.tcp_proxy_port.to_string(),
        args.request_timeout,
        args.env_path.clone(),
        args.client_pool_reserve,
    )
    .await?;

    // Interal TCP proxy address
    let tcp_proxy_address = format!("{}:{}", args.listen_address, args.tcp_proxy_port);

    // Create HTTP server configuration
    let http_config = ProxyConfig {
        listen_address: args.listen_address.clone(),
        listen_port: args.listen_port.clone(),
        tcp_proxy_url: tcp_proxy_address.clone(),
        timeout: Duration::from_secs(args.request_timeout),
        rpc_provider_url: rpc_provider_url.clone(),
    };

    // Run both services concurrently
    tokio::select! {
        result = proxy_client.run() => {
            if let Err(e) = result {
                eprintln!("TCP proxy client error: {}", e);
            }
        }
        result = start_proxy_server(http_config) => {
            if let Err(e) = result {
                eprintln!("HTTP proxy server error: {}", e);
            }
        }
    }

    Ok(())
}
