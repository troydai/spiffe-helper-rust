use anyhow::{Context, Result};
use axum::{http::StatusCode, response::IntoResponse, routing::get, Router};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::cli::HealthChecks;

const DEFAULT_LIVENESS_PATH: &str = "/health/live";
const DEFAULT_READINESS_PATH: &str = "/health/ready";

/// Status of a single credential type
#[derive(Debug, Clone, Default)]
pub struct CredentialStatus {
    /// Whether the last write operation succeeded
    pub write_succeeded: bool,
    /// When the credential was last successfully written
    pub last_success: Option<SystemTime>,
    /// Error message if last write failed
    pub last_error: Option<String>,
}

/// Aggregated health status for all credential types
#[derive(Debug, Clone, Default)]
pub struct HealthStatus {
    pub x509_svid: CredentialStatus,
    pub x509_bundle: Option<CredentialStatus>, // Only if bundle configured
    pub jwt_bundle: Option<CredentialStatus>,  // Only if JWT bundle configured
    pub jwt_svids: Vec<CredentialStatus>,      // One per configured JWT SVID
}

impl HealthStatus {
    /// Check if the helper is live (no recent failures)
    pub fn is_live(&self) -> bool {
        // Live if X.509 SVID write succeeded (at minimum)
        self.x509_svid.write_succeeded
            && self.x509_bundle.as_ref().is_none_or(|s| s.write_succeeded)
            && self.jwt_bundle.as_ref().is_none_or(|s| s.write_succeeded)
            && self.jwt_svids.iter().all(|s| s.write_succeeded)
    }

    /// Check if the helper is ready (all initial writes complete)
    pub fn is_ready(&self) -> bool {
        // Ready if all configured credentials have been written at least once
        self.x509_svid.last_success.is_some()
            && self
                .x509_bundle
                .as_ref()
                .is_none_or(|s| s.last_success.is_some())
            && self
                .jwt_bundle
                .as_ref()
                .is_none_or(|s| s.last_success.is_some())
            && self.jwt_svids.iter().all(|s| s.last_success.is_some())
    }
}

/// Thread-safe wrapper for sharing health status
pub type SharedHealthStatus = Arc<RwLock<HealthStatus>>;

/// Create a new shared health status instance
pub fn create_health_status() -> SharedHealthStatus {
    Arc::new(RwLock::new(HealthStatus::default()))
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn test_credential_status_default() {
        let status = CredentialStatus::default();
        assert!(!status.write_succeeded);
        assert!(status.last_success.is_none());
        assert!(status.last_error.is_none());
    }

    #[test]
    fn test_health_status_default() {
        let status = HealthStatus::default();
        assert!(!status.x509_svid.write_succeeded);
        assert!(status.x509_bundle.is_none());
        assert!(status.jwt_bundle.is_none());
        assert!(status.jwt_svids.is_empty());
    }

    #[test]
    fn test_is_live_all_succeeded() {
        let mut status = HealthStatus::default();
        status.x509_svid.write_succeeded = true;
        assert!(status.is_live());
    }

    #[test]
    fn test_is_live_x509_failed() {
        let mut status = HealthStatus::default();
        status.x509_svid.write_succeeded = false;
        assert!(!status.is_live());
    }

    #[test]
    fn test_is_live_with_bundle_succeeded() {
        let mut status = HealthStatus::default();
        status.x509_svid.write_succeeded = true;
        status.x509_bundle = Some(CredentialStatus {
            write_succeeded: true,
            last_success: None,
            last_error: None,
        });
        assert!(status.is_live());
    }

    #[test]
    fn test_is_live_with_bundle_failed() {
        let mut status = HealthStatus::default();
        status.x509_svid.write_succeeded = true;
        status.x509_bundle = Some(CredentialStatus {
            write_succeeded: false,
            last_success: None,
            last_error: None,
        });
        assert!(!status.is_live());
    }

    #[test]
    fn test_is_live_with_jwt_bundle_succeeded() {
        let mut status = HealthStatus::default();
        status.x509_svid.write_succeeded = true;
        status.jwt_bundle = Some(CredentialStatus {
            write_succeeded: true,
            last_success: None,
            last_error: None,
        });
        assert!(status.is_live());
    }

    #[test]
    fn test_is_live_with_jwt_bundle_failed() {
        let mut status = HealthStatus::default();
        status.x509_svid.write_succeeded = true;
        status.jwt_bundle = Some(CredentialStatus {
            write_succeeded: false,
            last_success: None,
            last_error: None,
        });
        assert!(!status.is_live());
    }

    #[test]
    fn test_is_live_with_jwt_svids_all_succeeded() {
        let mut status = HealthStatus::default();
        status.x509_svid.write_succeeded = true;
        status.jwt_svids = vec![
            CredentialStatus {
                write_succeeded: true,
                last_success: None,
                last_error: None,
            },
            CredentialStatus {
                write_succeeded: true,
                last_success: None,
                last_error: None,
            },
        ];
        assert!(status.is_live());
    }

    #[test]
    fn test_is_live_with_jwt_svids_one_failed() {
        let mut status = HealthStatus::default();
        status.x509_svid.write_succeeded = true;
        status.jwt_svids = vec![
            CredentialStatus {
                write_succeeded: true,
                last_success: None,
                last_error: None,
            },
            CredentialStatus {
                write_succeeded: false,
                last_success: None,
                last_error: None,
            },
        ];
        assert!(!status.is_live());
    }

    #[test]
    fn test_is_ready_not_ready() {
        let status = HealthStatus::default();
        assert!(!status.is_ready());
    }

    #[test]
    fn test_is_ready_x509_only() {
        let mut status = HealthStatus::default();
        status.x509_svid.last_success = Some(SystemTime::now());
        assert!(status.is_ready());
    }

    #[test]
    fn test_is_ready_with_bundle() {
        let mut status = HealthStatus::default();
        status.x509_svid.last_success = Some(SystemTime::now());
        status.x509_bundle = Some(CredentialStatus {
            write_succeeded: true,
            last_success: Some(SystemTime::now()),
            last_error: None,
        });
        assert!(status.is_ready());
    }

    #[test]
    fn test_is_ready_bundle_not_written() {
        let mut status = HealthStatus::default();
        status.x509_svid.last_success = Some(SystemTime::now());
        status.x509_bundle = Some(CredentialStatus {
            write_succeeded: true,
            last_success: None,
            last_error: None,
        });
        assert!(!status.is_ready());
    }

    #[test]
    fn test_is_ready_with_jwt_bundle() {
        let mut status = HealthStatus::default();
        status.x509_svid.last_success = Some(SystemTime::now());
        status.jwt_bundle = Some(CredentialStatus {
            write_succeeded: true,
            last_success: Some(SystemTime::now()),
            last_error: None,
        });
        assert!(status.is_ready());
    }

    #[test]
    fn test_is_ready_with_jwt_svids() {
        let mut status = HealthStatus::default();
        status.x509_svid.last_success = Some(SystemTime::now());
        status.jwt_svids = vec![
            CredentialStatus {
                write_succeeded: true,
                last_success: Some(SystemTime::now()),
                last_error: None,
            },
            CredentialStatus {
                write_succeeded: true,
                last_success: Some(SystemTime::now()),
                last_error: None,
            },
        ];
        assert!(status.is_ready());
    }

    #[test]
    fn test_is_ready_jwt_svid_not_written() {
        let mut status = HealthStatus::default();
        status.x509_svid.last_success = Some(SystemTime::now());
        status.jwt_svids = vec![
            CredentialStatus {
                write_succeeded: true,
                last_success: Some(SystemTime::now()),
                last_error: None,
            },
            CredentialStatus {
                write_succeeded: true,
                last_success: None,
                last_error: None,
            },
        ];
        assert!(!status.is_ready());
    }

    #[tokio::test]
    async fn test_create_health_status() {
        let status = create_health_status();
        let guard = status.read().await;
        assert!(!guard.x509_svid.write_succeeded);
        assert!(guard.x509_bundle.is_none());
    }
}
