use anyhow::{Context, Result};
use spiffe::bundle::BundleSource;
use spiffe::svid::SvidSource;
use spiffe::workload_api::x509_source::X509Source;
use std::sync::Arc;
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{interval, Duration};

use crate::config::Config;
use crate::health;
use crate::workload_api;

const DEFAULT_LIVENESS_LOG_INTERVAL_SECS: u64 = 30;

/// Runs the daemon mode: fetches initial certificate, starts health server,
/// and waits for SIGTERM.
pub async fn run(config: Config, agent_address: String) -> Result<()> {
    println!("Starting spiffe-helper-rust daemon...");

    // Create X509Source (this waits for the first update)
    let source = workload_api::create_x509_source(&agent_address).await?;
    println!("Connected to SPIRE agent");

    // Initial fetch and write
    fetch_and_process_update(&source, &config).await?;

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

    // Watch for updates
    let mut update_channel = source.updated();

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
            // Watch for updates from X509Source
            res = update_channel.changed() => {
                 if let Err(e) = res {
                     eprintln!("Update channel closed: {}", e);
                     break Err(anyhow::anyhow!("X509Source update channel closed"));
                 }

                 println!("Received X.509 update notification");
                 if let Err(e) = fetch_and_process_update(&source, &config).await {
                     eprintln!("Failed to handle X.509 update: {}", e);
                 }
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

async fn fetch_and_process_update(source: &Arc<X509Source>, config: &Config) -> Result<()> {
    let svid = source
        .get_svid()
        .map_err(|e| anyhow::anyhow!("Failed to get SVID: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("No SVID received"))?;

    let bundle = source
        .get_bundle_for_trust_domain(svid.spiffe_id().trust_domain())
        .map_err(|e| anyhow::anyhow!("Failed to get bundle: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("No bundle received"))?;

    workload_api::on_x509_update(&svid, &bundle, config).await
}
