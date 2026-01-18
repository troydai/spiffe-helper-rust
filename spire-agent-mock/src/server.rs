use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use crate::svid::{SvidConfig, SvidGenerator};

pub mod workload {
    tonic::include_proto!("_");
}

use workload::spiffe_workload_api_server::SpiffeWorkloadApi;
pub use workload::spiffe_workload_api_server::SpiffeWorkloadApiServer;
use workload::{
    JwtBundlesRequest, JwtBundlesResponse, JwtsvidRequest, JwtsvidResponse, ValidateJwtsvidRequest,
    ValidateJwtsvidResponse, X509BundlesRequest, X509BundlesResponse, X509svid, X509svidRequest,
    X509svidResponse,
};

pub struct MockWorkloadApi {
    svid_generator: Arc<SvidGenerator>,
    rotation_interval: Duration,
}

impl MockWorkloadApi {
    pub fn new() -> Self {
        let default_interval = Duration::from_secs(SvidConfig::default().ttl_seconds as u64);
        Self::with_config_and_rotation(SvidConfig::default(), default_interval)
    }

    #[allow(dead_code)]
    pub fn with_config(config: SvidConfig) -> Self {
        let rotation_interval = Duration::from_secs(config.ttl_seconds as u64);
        Self::with_config_and_rotation(config, rotation_interval)
    }

    pub fn with_rotation_interval(rotation_interval: Duration) -> Self {
        Self::with_config_and_rotation(SvidConfig::default(), rotation_interval)
    }

    pub fn with_config_and_rotation(config: SvidConfig, rotation_interval: Duration) -> Self {
        Self {
            svid_generator: Arc::new(SvidGenerator::new(config)),
            rotation_interval,
        }
    }
}

impl Default for MockWorkloadApi {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl SpiffeWorkloadApi for MockWorkloadApi {
    type FetchX509SVIDStream = Pin<Box<dyn Stream<Item = Result<X509svidResponse, Status>> + Send>>;

    async fn fetch_x509svid(
        &self,
        _request: Request<X509svidRequest>,
    ) -> Result<Response<Self::FetchX509SVIDStream>, Status> {
        println!("Received FetchX509SVID request");

        let svid_generator = Arc::clone(&self.svid_generator);
        let rotation_interval = self.rotation_interval;

        let stream = async_stream::stream! {
            loop {
                let svid = svid_generator.generate_svid();

                let x509_svid = X509svid {
                    spiffe_id: svid.spiffe_id.clone(),
                    x509_svid: svid.cert_chain_der,
                    x509_svid_key: svid.private_key_der,
                    bundle: svid.bundle_der,
                    hint: String::new(),
                };

                let response = X509svidResponse {
                    svids: vec![x509_svid],
                    crl: vec![],
                    federated_bundles: std::collections::HashMap::new(),
                };

                println!("Sending X509SVID: {}", svid.spiffe_id);
                yield Ok(response);

                // Wait for the rotation interval before sending the next certificate
                tokio::time::sleep(rotation_interval).await;
            }
        };

        Ok(Response::new(Box::pin(stream)))
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
