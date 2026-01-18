mod server;
mod svid;

use anyhow::Result;
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;

use server::{MockWorkloadApi, SpiffeWorkloadApiServer};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Unix Domain Socket path to listen on
    #[arg(
        short,
        long,
        default_value = "/tmp/agent.sock",
        env = "SPIFFE_ENDPOINT_SOCKET"
    )]
    socket_path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let socket_path = args.socket_path;

    // Remove existing socket file if it exists
    if socket_path.exists() {
        fs::remove_file(&socket_path)?;
    }

    // Create parent directory if it doesn't exist
    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent)?;
    }

    println!(
        "SPIRE Agent Mock listening on uds://{}",
        socket_path.display()
    );

    let uds = UnixListener::bind(&socket_path)?;
    let uds_stream = UnixListenerStream::new(uds);

    let service = MockWorkloadApi::new();

    Server::builder()
        .add_service(SpiffeWorkloadApiServer::new(service))
        .serve_with_incoming(uds_stream)
        .await?;

    Ok(())
}
