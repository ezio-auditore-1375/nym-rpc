use anyhow::Result;
use clap::Parser;
use nym_rpc::common::{KNOWN_EXIT_NODE_API_URLS, RpcProviderUrl, get_nym_address_from_api};
use nym_rpc::http_proxy_server::{HttpProxyConfig, start_http_proxy_server};
use nym_rpc::tcp_proxy_client::TcpProxyClient;
use nym_sdk::mixnet::Recipient;
use rand::Rng;
use tracing::debug;

#[derive(Parser, Debug)]
#[command(name = "nym-rpc-client")]
#[command(
    about = "Receives RPC requests and forwards them through the Nym mixnet to the specified RPC provider"
)]
struct Args {
    // listen address for HTTP proxy
    #[clap(long, default_value = "127.0.0.1")]
    listen_address: String,

    // listen port for HTTP proxy
    #[clap(long, default_value = "8545")]
    listen_port: u16,

    // listen port for TCP proxy
    #[clap(long, default_value = "8118")]
    tcp_proxy_port: u16,

    // response timeout in seconds
    #[clap(long, default_value_t = 60)]
    request_timeout: u64,

    // rpc provider URL (e.g., https://mainnet.infura.io/v3/YOUR_PROJECT_ID)
    #[clap(short = 'r', long)]
    rpc_provider: String,

    // where to forward the RPC requests to (nym-rpc-server nym address)
    // if none provided we randomly pick a known exit node
    #[clap(short = 'x', long)]
    exit_node_address: Option<String>,

    // how many clients to have running in reserve
    #[clap(long, default_value_t = 2)]
    client_pool_reserve: usize,

    // nym env filepath to create the nym client with
    // if none provided we use the default mainnet env file
    #[clap(short, long)]
    env_path: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // we are launching a TCP proxy server and an HTTP proxy server
    // user connects to the HTTP proxy server
    // and the HTTP proxy server forwards the requests to the TCP proxy server

    nym_bin_common::logging::setup_tracing_logger();
    let args = Args::parse();

    // if exit node address is provided use it
    // otherwise pick a random known exit node
    // and get the Nym address from the API
    let exit_node_address: Recipient = if let Some(address) = args.exit_node_address {
        Recipient::try_from_base58_string(&address).expect("Invalid server address")
    } else {
        let exit_node =
            KNOWN_EXIT_NODE_API_URLS[rand::rng().random_range(0..KNOWN_EXIT_NODE_API_URLS.len())];
        get_nym_address_from_api(exit_node).await
    };
    debug!("using exit node address: {}", exit_node_address);

    // extract HOST from RPC provider URL
    let rpc_provider = RpcProviderUrl::new(&args.rpc_provider);

    // create TCP proxy client
    let proxy_client = TcpProxyClient::new(
        exit_node_address,
        rpc_provider.full_host.clone(),
        &args.listen_address,
        &args.tcp_proxy_port.to_string(),
        args.request_timeout,
        args.env_path.clone(),
        args.client_pool_reserve,
    )
    .await?;

    // create HTTP server configuration
    let http_config = HttpProxyConfig {
        listen_address: args.listen_address,
        listen_port: args.listen_port,
        tcp_proxy_port: args.tcp_proxy_port,
        request_timeout: args.request_timeout,
        rpc_provider: rpc_provider,
    };

    // run both services concurrently
    tokio::select! {
        result = proxy_client.run() => {
            if let Err(e) = result {
                eprintln!("TCP proxy client error: {}", e);
            }
        }
        result = start_http_proxy_server(http_config) => {
            if let Err(e) = result {
                eprintln!("HTTP proxy server error: {}", e);
            }
        }
    }

    Ok(())
}
