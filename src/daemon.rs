use anyhow::{Context, Result};
use spiffe::svid::SvidSource;
use std::path::PathBuf;
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{sleep, Duration};

use crate::config::Config;
use crate::health;
use crate::workload_api;

const DEFAULT_LIVENESS_LOG_INTERVAL_SECS: u64 = 30;

/// Runs the daemon mode: fetches initial certificate, starts health server,
/// watches for certificate updates, and waits for SIGTERM.
pub async fn run(config: Config) -> Result<()> {
    println!("Starting spiffe-helper-rust daemon...");

    // Validate required configuration
    let agent_address = config
        .agent_address
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("agent_address must be configured"))?;
    let cert_dir = config
        .cert_dir
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("cert_dir must be configured"))?;

    // Create cert directory if it doesn't exist
    let cert_dir_path = PathBuf::from(cert_dir);
    std::fs::create_dir_all(&cert_dir_path)
        .with_context(|| format!("Failed to create cert directory: {}", cert_dir))?;

    // Create X509Source for continuous certificate watching
    println!("Connecting to SPIRE agent at {agent_address}...");
    let x509_source = workload_api::create_x509_source(agent_address).await?;

    // Fetch and write initial certificate
    let svid = x509_source
        .get_svid()
        .map_err(|e| anyhow::anyhow!("Failed to get initial SVID: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("X509Source returned no SVID (None)"))?;

    workload_api::on_x509_update(
        &svid,
        &cert_dir_path,
        config.svid_file_name(),
        config.svid_key_file_name(),
    )?;

    println!("Initial certificate written successfully");

    // Calculate initial refresh interval
    let mut refresh_interval = workload_api::calculate_refresh_interval(&svid);
    println!("Certificate refresh scheduled in {:?}", refresh_interval);

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
    let liveness_interval = Duration::from_secs(DEFAULT_LIVENESS_LOG_INTERVAL_SECS);
    let mut last_liveness = tokio::time::Instant::now();

    println!("Daemon running. Waiting for SIGTERM to shutdown...");

    // Main daemon loop with certificate rotation
    let result = loop {
        // Calculate time until next refresh and next liveness log
        let time_until_refresh = refresh_interval;
        let time_until_liveness = liveness_interval.saturating_sub(last_liveness.elapsed());

        tokio::select! {
            _ = sigterm.recv() => {
                println!("Received SIGTERM, shutting down gracefully...");
                break Ok(());
            }

            _ = sleep(time_until_refresh) => {
                // Time to check for certificate updates
                match x509_source.get_svid() {
                    Ok(Some(new_svid)) => {
                        // Write updated certificate
                        match workload_api::on_x509_update(
                            &new_svid,
                            &cert_dir_path,
                            config.svid_file_name(),
                            config.svid_key_file_name(),
                        ) {
                            Ok(()) => {
                                // Recalculate refresh interval based on new certificate
                                refresh_interval = workload_api::calculate_refresh_interval(&new_svid);
                                println!(
                                    "Certificate updated successfully. Next refresh in {:?}",
                                    refresh_interval
                                );
                            }
                            Err(e) => {
                                eprintln!("Failed to write updated certificate: {e}");
                                // Retry sooner on write failure
                                refresh_interval = Duration::from_secs(5);
                            }
                        }
                    }
                    Ok(None) => {
                        eprintln!("X509Source returned no SVID");
                        // Retry sooner when SVID is missing
                        refresh_interval = Duration::from_secs(5);
                    }
                    Err(e) => {
                        eprintln!("Failed to get SVID from X509Source: {e}");
                        // Retry sooner on error
                        refresh_interval = Duration::from_secs(5);
                    }
                }
            }

            _ = sleep(time_until_liveness) => {
                println!("spiffe-helper-rust daemon is alive");
                last_liveness = tokio::time::Instant::now();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_daemon_run_missing_agent_address() {
        let config = Config {
            agent_address: None,
            cert_dir: Some("/tmp/certs".to_string()),
            ..Default::default()
        };

        let result = run(config).await;
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("agent_address must be configured"));
    }

    #[tokio::test]
    async fn test_daemon_run_missing_cert_dir() {
        let config = Config {
            agent_address: Some("unix:///tmp/agent.sock".to_string()),
            cert_dir: None,
            ..Default::default()
        };

        let result = run(config).await;
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("cert_dir must be configured"));
    }
}
