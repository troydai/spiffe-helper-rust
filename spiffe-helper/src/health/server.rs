use anyhow::{Context, Result};
use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration, MissedTickBehavior};

use crate::cli::HealthChecks;

/// A handle to the health check server.
pub enum HealthCheckServer {
    Disabled,
    Enabled {
        server_handle: JoinHandle<Result<()>>,
        heartbeat_handle: JoinHandle<()>,
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
                server_handle: _,
                heartbeat_handle,
                receiver,
            } => match receiver.await {
                Ok(res) => {
                    if !heartbeat_handle.is_finished() {
                        heartbeat_handle.abort();
                    }
                    res
                }
                Err(_) => Err(anyhow::anyhow!("Health check server task disappeared")),
            },
        }
    }

    /// Shuts down the health check server if it is running.
    pub fn shutdown(&mut self) {
        match self {
            HealthCheckServer::Disabled => (),
            HealthCheckServer::Enabled {
                server_handle,
                heartbeat_handle,
                receiver: _,
            } => {
                if !server_handle.is_finished() {
                    server_handle.abort();
                    println!("Health check server stopped");
                }
                if !heartbeat_handle.is_finished() {
                    heartbeat_handle.abort();
                }
            }
        }
    }

    /// Returns true if the health check server is enabled.
    #[must_use]
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

async fn heartbeat_reporter() {
    let mut liveness_interval = interval(Duration::from_secs(30));
    liveness_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        liveness_interval.tick().await;
        println!("spiffe-helper daemon is alive");
    }
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

    let server_handle = tokio::spawn(async move {
        let res = axum::serve(listener, app)
            .await
            .context("Health check server stopped");

        let signal = res.as_ref().cloned().map_err(|e| anyhow::anyhow!("{e}"));
        let _ = tx.send(signal);

        res
    });

    let heartbeat_handle = tokio::spawn(heartbeat_reporter());

    let server = HealthCheckServer::Enabled {
        server_handle,
        heartbeat_handle,
        receiver: rx,
    };

    Ok(server)
}
