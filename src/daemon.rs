use anyhow::{Context, Result};
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{interval, Duration};

use crate::config::Config;
use crate::health;
use crate::svid;

const DEFAULT_LIVENESS_LOG_INTERVAL_SECS: u64 = 30;

/// Runs the daemon mode: fetches initial certificate, starts health server,
/// and waits for SIGTERM.
pub async fn run(config: Config) -> Result<()> {
    println!("Starting spiffe-helper-rust daemon...");

    // Fetch initial X.509 SVID at startup
    svid::fetch_x509_certificate(&config).await?;

    // Start health check server if enabled
    let health_checks = config.health_checks.clone();
    let mut health_server_handle = match &health_checks {
        Some(hc) => health::start_server(hc).await?,
        None => None,
    };

    // Set up signal handling for graceful shutdown
    let mut sigterm =
        signal(SignalKind::terminate()).context("Failed to register SIGTERM handler")?;

    // Set up periodic liveness logging
    let mut liveness_interval = interval(Duration::from_secs(DEFAULT_LIVENESS_LOG_INTERVAL_SECS));
    liveness_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    println!("Daemon running. Waiting for SIGTERM to shutdown...");

    // Main daemon loop
    let result = loop {
        tokio::select! {
            _ = sigterm.recv() => {
                println!("Received SIGTERM, shutting down gracefully...");
                break Ok(());
            }
            _ = liveness_interval.tick() => {
                println!("spiffe-helper-rust daemon is alive");
            }
            // If health server is running, watch for its completion (which indicates failure)
            Some(res) = async {
                match &mut health_server_handle {
                    Some(handle) => Some(handle.await),
                    None => std::future::pending().await,
                }
            } => {
                match res {
                    Ok(Ok(())) => {
                        // Server exited cleanly (shouldn't happen normally)
                        println!("Health check server exited unexpectedly");
                        break Ok(());
                    }
                    Ok(Err(e)) => {
                        // Server returned an error
                        break Err(e);
                    }
                    Err(e) => {
                        // Task panicked or was cancelled
                        break Err(anyhow::anyhow!("Health check server task failed: {e}"));
                    }
                }
            }
        }
    };

    // Shutdown health check server if it was started and still running
    if let Some(ref handle) = health_server_handle {
        if !handle.is_finished() {
            handle.abort();
            println!("Health check server stopped");
        }
    }

    println!("Daemon shutdown complete");
    result
}
