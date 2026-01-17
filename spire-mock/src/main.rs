use clap::Parser;
use std::path::PathBuf;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::NamedService;
use tonic::{transport::Server, Request, Response, Status};
use tower_http::trace::TraceLayer;

pub mod spiffe {
    pub mod workload {
        // tonic::include_proto!("spiffe.workload");
        include!(concat!(env!("OUT_DIR"), "/_.rs"));
    }
}

use spiffe::workload::spiffe_workload_api_server::{SpiffeWorkloadApi, SpiffeWorkloadApiServer};
use spiffe::workload::{
    JwtBundlesRequest, JwtBundlesResponse, JwtsvidRequest, JwtsvidResponse, ValidateJwtsvidRequest,
    ValidateJwtsvidResponse, X509svid, X509svidRequest, X509svidResponse,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the Unix Domain Socket
    #[arg(long, default_value = "/tmp/spire-agent/public/api.sock")]
    socket_path: PathBuf,

    /// Path to the X.509 SVID certificate (PEM or DER)
    #[arg(long)]
    cert_path: PathBuf,

    /// Path to the Private Key (PEM or DER)
    #[arg(long)]
    key_path: PathBuf,

    /// Path to the Trust Bundle (PEM or DER)
    #[arg(long)]
    bundle_path: PathBuf,

    /// SPIFFE ID
    #[arg(long)]
    spiffe_id: String,
}

#[derive(Debug)]
struct MockWorkloadApi {
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    bundle_der: Vec<u8>,
    spiffe_id: String,
}

#[tonic::async_trait]
impl SpiffeWorkloadApi for MockWorkloadApi {
    type FetchX509SVIDStream =
        tokio_stream::wrappers::ReceiverStream<Result<X509svidResponse, Status>>;
    type FetchJWTSVIDStream =
        tokio_stream::wrappers::ReceiverStream<Result<JwtsvidResponse, Status>>;
    type FetchJWTBundlesStream =
        tokio_stream::wrappers::ReceiverStream<Result<JwtBundlesResponse, Status>>;

    async fn fetch_x509svid(
        &self,
        request: Request<X509svidRequest>,
    ) -> Result<Response<Self::FetchX509SVIDStream>, Status> {
        println!("Received FetchX509SVID request: {:?}", request);
        let (tx, rx) = tokio::sync::mpsc::channel(1);

        let svid = X509svid {
            spiffe_id: self.spiffe_id.clone(),
            x509_svid: self.cert_der.clone(),
            x509_svid_key: self.key_der.clone(),
            bundle: self.bundle_der.clone(),
            hint: "".to_string(),
        };

        let mut bundles = std::collections::HashMap::new();
        // Extract trust domain from spiffe_id
        let trust_domain = self
            .spiffe_id
            .split("://")
            .nth(1)
            .unwrap_or("")
            .split('/')
            .next()
            .unwrap_or("example.org")
            .to_string();
        bundles.insert(trust_domain, self.bundle_der.clone());

        let response = X509svidResponse {
            svids: vec![svid],
            bundles,
            crl_bundles: std::collections::HashMap::new(),
        };

        tokio::spawn(async move {
            if let Err(e) = tx.send(Ok(response)).await {
                eprintln!("Error sending response: {:?}", e);
                return;
            }
            // Keep the stream open indefinitely
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
            loop {
                interval.tick().await;
            }
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
            rx,
        )))
    }

    async fn fetch_jwtsvid(
        &self,
        _request: Request<JwtsvidRequest>,
    ) -> Result<Response<Self::FetchJWTSVIDStream>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn fetch_jwt_bundles(
        &self,
        _request: Request<JwtBundlesRequest>,
    ) -> Result<Response<Self::FetchJWTBundlesStream>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }

    async fn validate_jwtsvid(
        &self,
        _request: Request<ValidateJwtsvidRequest>,
    ) -> Result<Response<ValidateJwtsvidResponse>, Status> {
        Err(Status::unimplemented("Not implemented"))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let args = Args::parse();

    let read_pem_to_der = |path: &PathBuf| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let content = std::fs::read(path)?;
        // Check if it's PEM (starts with hyphen)
        if content.starts_with(b"-") {
            let p = pem::parse(&content)?;
            Ok(p.contents.to_vec())
        } else {
            Ok(content)
        }
    };

    let cert_der = read_pem_to_der(&args.cert_path)?;
    let key_der = read_pem_to_der(&args.key_path)?;
    let bundle_der = read_pem_to_der(&args.bundle_path)?;

    let service = MockWorkloadApi {
        cert_der,
        key_der,
        bundle_der,
        spiffe_id: args.spiffe_id,
    };

    // Remove existing socket
    if args.socket_path.exists() {
        std::fs::remove_file(&args.socket_path)?;
    }
    // Create parent dir if not exists
    if let Some(parent) = args.socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let uds = UnixListener::bind(&args.socket_path)?;
    let stream = UnixListenerStream::new(uds);

    println!("Listening on {:?}", args.socket_path);
    println!(
        "Registered service: {}",
        <SpiffeWorkloadApiServer<MockWorkloadApi> as NamedService>::NAME
    );

    Server::builder()
        .layer(TraceLayer::new_for_grpc())
        .add_service(SpiffeWorkloadApiServer::new(service))
        .serve_with_incoming(stream)
        .await?;

    Ok(())
}
