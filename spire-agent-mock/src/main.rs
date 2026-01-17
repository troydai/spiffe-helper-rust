use anyhow::Result;
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::pin::Pin;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tokio_stream::Stream;
use tonic::{transport::Server, Request, Response, Status};

pub mod workload {
    tonic::include_proto!("_");
}

use workload::spiffe_workload_api_server::{SpiffeWorkloadApi, SpiffeWorkloadApiServer};
use workload::{
    JwtBundlesRequest, JwtBundlesResponse, JwtsvidRequest, JwtsvidResponse, ValidateJwtsvidRequest,
    ValidateJwtsvidResponse, X509BundlesRequest, X509BundlesResponse, X509svidRequest,
    X509svidResponse,
};

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

pub struct MyWorkloadApi {}

#[tonic::async_trait]
impl SpiffeWorkloadApi for MyWorkloadApi {
    type FetchX509SVIDStream = Pin<Box<dyn Stream<Item = Result<X509svidResponse, Status>> + Send>>;

    async fn fetch_x509svid(
        &self,
        _request: Request<X509svidRequest>,
    ) -> Result<Response<Self::FetchX509SVIDStream>, Status> {
        println!("Received FetchX509SVID request");
        Err(Status::unimplemented("not implemented"))
    }

    type FetchX509BundlesStream =
        Pin<Box<dyn Stream<Item = Result<X509BundlesResponse, Status>> + Send>>;

    async fn fetch_x509_bundles(
        &self,
        _request: Request<X509BundlesRequest>,
    ) -> Result<Response<Self::FetchX509BundlesStream>, Status> {
        println!("Received FetchX509Bundles request");
        Err(Status::unimplemented("not implemented"))
    }

    async fn fetch_jwtsvid(
        &self,
        _request: Request<JwtsvidRequest>,
    ) -> Result<Response<JwtsvidResponse>, Status> {
        println!("Received FetchJWTSVID request");
        Err(Status::unimplemented("not implemented"))
    }

    type FetchJWTBundlesStream =
        Pin<Box<dyn Stream<Item = Result<JwtBundlesResponse, Status>> + Send>>;

    async fn fetch_jwt_bundles(
        &self,
        _request: Request<JwtBundlesRequest>,
    ) -> Result<Response<Self::FetchJWTBundlesStream>, Status> {
        println!("Received FetchJWTBundles request");
        Err(Status::unimplemented("not implemented"))
    }

    async fn validate_jwtsvid(
        &self,
        _request: Request<ValidateJwtsvidRequest>,
    ) -> Result<Response<ValidateJwtsvidResponse>, Status> {
        println!("Received ValidateJWTSVID request");
        Err(Status::unimplemented("not implemented"))
    }
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

    let service = MyWorkloadApi {};

    Server::builder()
        .add_service(SpiffeWorkloadApiServer::new(service))
        .serve_with_incoming(uds_stream)
        .await?;

    Ok(())
}
