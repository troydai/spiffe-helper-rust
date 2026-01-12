use anyhow::{Context, Result};
use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::cli::HealthChecks;

/// A handle to the health check server.
pub enum HealthCheckServer {
    Disabled,
    Enabled {
        handle: JoinHandle<Result<()>>,
        receiver: oneshot::Receiver<Result<()>>,
    },
}

impl HealthCheckServer {
    pub async fn new(health_checks: Option<&HealthChecks>) -> Result<Self> {
        match health_checks {
            None => Ok(Self::Disabled),
            Some(hc) => start(hc).await,
        }
    }

    /// Waits for the health check server to exit.
    ///
    /// If health checks are disabled, this future will never complete.
    pub async fn wait(&mut self) -> Result<()> {
        match self {
            HealthCheckServer::Disabled => std::future::pending().await,
            HealthCheckServer::Enabled {
                handle: _,
                receiver,
            } => match receiver.await {
                Ok(res) => res,
                Err(_) => Err(anyhow::anyhow!("Health check server task disappeared")),
            },
        }
    }

    /// Shuts down the health check server if it is running.
    pub fn shutdown(&mut self) {
        match self {
            HealthCheckServer::Disabled => (),
            HealthCheckServer::Enabled {
                handle,
                receiver: _,
            } => {
                if !handle.is_finished() {
                    handle.abort();
                    println!("Health check server stopped");
                }
            }
        }
    }

    /// Returns true if the health check server is enabled.
    pub fn is_enabled(&self) -> bool {
        matches!(self, HealthCheckServer::Enabled { .. })
    }
}

async fn liveness_handler() -> impl IntoResponse {
    StatusCode::OK
}

async fn readiness_handler() -> impl IntoResponse {
    StatusCode::OK
}

/// Starts the health check HTTP server if enabled in configuration.
async fn start(hc: &HealthChecks) -> Result<HealthCheckServer> {
    let (tx, rx) = oneshot::channel();
    let addr = hc.bind_addr();
    let liveness = hc.liveness_path();
    let readiness = hc.readiness_path();

    println!("Starting health check server on {addr}");
    println!("  Liveness path: {liveness}");
    println!("  Readiness path: {readiness}");

    let app = Router::new()
        .route(&liveness, get(liveness_handler))
        .route(&readiness, get(readiness_handler));

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("Failed to bind to {addr}"))?;

    let handle = tokio::spawn(async move {
        let res = axum::serve(listener, app)
            .await
            .context("Health check server stopped");

        let signal = res.as_ref().cloned().map_err(|e| anyhow::anyhow!("{e}"));
        let _ = tx.send(signal);

        res
    });

    let server = HealthCheckServer::Enabled {
        handle,
        receiver: rx,
    };

    Ok(server)
}
