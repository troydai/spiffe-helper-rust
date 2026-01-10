use anyhow::{Context, Result};
use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use tokio::task::JoinHandle;

use crate::cli::HealthChecks;

const DEFAULT_LIVENESS_PATH: &str = "/health/live";
const DEFAULT_READINESS_PATH: &str = "/health/ready";

pub async fn liveness_handler() -> impl IntoResponse {
    StatusCode::OK
}

pub async fn readiness_handler() -> impl IntoResponse {
    StatusCode::OK
}

/// Starts the health check HTTP server if enabled in configuration.
/// Returns a JoinHandle for the server task, or None if health checks are disabled.
pub async fn start_server(health_checks: &HealthChecks) -> Result<Option<JoinHandle<Result<()>>>> {
    if !health_checks.listener_enabled {
        return Ok(None);
    }

    let bind_addr = format!("0.0.0.0:{}", health_checks.bind_port);
    let liveness_path = health_checks
        .liveness_path
        .clone()
        .unwrap_or_else(|| DEFAULT_LIVENESS_PATH.to_string());
    let readiness_path = health_checks
        .readiness_path
        .clone()
        .unwrap_or_else(|| DEFAULT_READINESS_PATH.to_string());

    println!("Starting health check server on {bind_addr}");
    println!("  Liveness path: {liveness_path}");
    println!("  Readiness path: {readiness_path}");

    let app = Router::new()
        .route(&liveness_path, get(liveness_handler))
        .route(&readiness_path, get(readiness_handler));

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("Failed to bind to {bind_addr}"))?;

    Ok(Some(tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .context("Health check server failed")
    })))
}
