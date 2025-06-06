use anyhow::Result;
use clap::Parser;
use nym_rpc::tcp_proxy_server::TcpProxyServer;

#[derive(Parser, Debug)]
struct Args {
    /// Config directory
    #[clap(short, long, default_value = "/tmp/nym-tcp-proxy-server")]
    config_dir: String,

    /// Optional env filepath - if none is supplied then the proxy defaults to using mainnet else just use a path to one of the supplied files in envs/ e.g. ./envs/sandbox.env
    #[clap(short, long)]
    env_path: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    nym_bin_common::logging::setup_tracing_logger();

    let args = Args::parse();

    let home_dir = dirs::home_dir().expect("Unable to get home directory");
    let conf_path = format!("{}{}", home_dir.display(), args.config_dir);

    let mut proxy_server = TcpProxyServer::new(&conf_path, args.env_path.clone()).await?;

    proxy_server.run_with_shutdown().await
}
