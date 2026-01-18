use std::pin::Pin;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

pub mod workload {
    tonic::include_proto!("_");
}

use workload::spiffe_workload_api_server::SpiffeWorkloadApi;
pub use workload::spiffe_workload_api_server::SpiffeWorkloadApiServer;
use workload::{
    JwtBundlesRequest, JwtBundlesResponse, JwtsvidRequest, JwtsvidResponse, ValidateJwtsvidRequest,
    ValidateJwtsvidResponse, X509BundlesRequest, X509BundlesResponse, X509svidRequest,
    X509svidResponse,
};

pub struct MockWorkloadApi {}

impl MockWorkloadApi {
    pub fn new() -> Self {
        Self {}
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
