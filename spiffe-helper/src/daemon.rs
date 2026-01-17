use anyhow::{Context, Result};
use spiffe::bundle::BundleSource;
use spiffe::X509Source;
use std::path::Path;
use std::process::ExitStatus;
use std::sync::Arc;
use tokio::process::Command;
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{interval, Duration};

use crate::cli::Config;
use crate::health;
use crate::process;
use crate::signal;
use crate::workload_api;

const DEFAULT_LIVENESS_LOG_INTERVAL_SECS: u64 = 30;

/// Runs the daemon mode: fetches initial certificate, starts health server,
/// and waits for SIGTERM.
pub async fn run(config: Config) -> Result<()> {
    println!("Starting spiffe-helper daemon...");

    // Parse renew signal if configured
    let renew_signal = config
        .renew_signal
        .as_ref()
        .map(|s| signal::parse_signal_name(s))
        .transpose()
        .context("Failed to parse renew_signal")?;

    // Create X509Source (this waits for the first update)
    let source = workload_api::X509SourceFactory::new()
        .with_address(config.agent_address()?)
        .create()
        .await?;
    println!("Connected to SPIRE agent");

    // Initial fetch and write
    fetch_and_process_update(&source, &config)?;

    // Spawn managed child process if configured
    let mut child = if let Some(cmd) = &config.cmd {
        let mut command = Command::new(cmd);
        if let Some(args_str) = &config.cmd_args {
            let args = process::parse_cmd_args(args_str)?;
            command.args(args);
        }
        println!(
            "Spawning managed process: {cmd} {:?}",
            config.cmd_args.as_deref().unwrap_or("")
        );
        Some(command.spawn().context("Failed to spawn managed process")?)
    } else {
        None
    };

    let mut health_server = health::HealthCheckServer::new(config.health_checks.as_ref()).await?;

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
                println!("spiffe-helper daemon is alive");
            }
            // Watch for updates from X509Source
            res = update_channel.changed() => {
                 if let Err(e) = res {
                     eprintln!("Update channel closed: {e}");
                     break Err(anyhow::anyhow!("X509Source update channel closed"));
                 }

                 println!("Received X.509 update notification");
                 if let Err(e) = fetch_and_process_update(&source, &config) {
                     eprintln!("Failed to handle X.509 update: {e}");
                 } else {
                     // Successfully updated certificates, now send signal if configured
                     if let Some(sig) = renew_signal {
                         // Signal managed child process
                         if let Some(ref c) = child {
                             if let Some(pid) = c.id() {
                                 println!("Sending signal {sig:?} to managed process (PID: {pid})");
                                 match i32::try_from(pid) {
                                     Ok(pid_i32) => {
                                         if let Err(e) = signal::send_signal(pid_i32, sig) {
                                             eprintln!("Failed to signal managed process: {e}");
                                         }
                                     }
                                     Err(e) => {
                                         eprintln!("Failed to convert PID {pid} to i32: {e}");
                                     }
                                 }
                             }
                         }

                         // Signal process via PID file
                         if let Some(pid_file) = &config.pid_file_name {
                             match signal::read_pid_from_file(Path::new(pid_file)) {
                                 Ok(pid) => {
                                     println!("Sending signal {sig:?} to process from PID file {pid_file} (PID: {pid})");
                                     if let Err(e) = signal::send_signal(pid, sig) {
                                         eprintln!("Failed to signal process from PID file: {e}");
                                     }
                                 }
                                 Err(e) => {
                                     eprintln!("Failed to read PID from file {pid_file}: {e}");
                                 }
                             }
                         }
                     }
                 }
            }
            // Watch for child process exit if it's being managed
            Some(status) = async {
                match child.as_mut() {
                    Some(c) => Some(c.wait().await),
                    None => None,
                }
            }, if child.is_some() => {
                let status_str = match status {
                    Ok(s) => (s as ExitStatus).to_string(),
                    Err(e) => format!("error: {e}"),
                };
                println!("Managed process exited: {status_str}");
                // Depending on requirements, we might want to restart it or exit.
                // For now, we'll just stop managing it and continue running the daemon.
                child = None;
            }
            // If health server is running, watch for its completion (which indicates failure)
            res = health_server.wait(), if health_server.is_enabled() => {
                match res {
                    Ok(()) => {
                        // Server exited cleanly (shouldn't happen normally)
                        println!("Health check server exited unexpectedly");
                        break Ok(());
                    }
                    Err(e) => {
                        // Server returned an error
                        break Err(e);
                    }
                }
            }
        }
    };

    // Shutdown child process if it was started and still running
    if let Some(mut c) = child {
        println!("Stopping managed process...");
        let _ = c.kill().await;
    }

    // Shutdown health check server if it was started and still running
    health_server.shutdown();

    println!("Daemon shutdown complete");
    result
}

fn fetch_and_process_update(source: &Arc<X509Source>, config: &Config) -> Result<()> {
    let svid = source
        .svid()
        .map_err(|e| anyhow::anyhow!("Failed to get SVID: {e}"))?;

    let bundle = source
        .bundle_for_trust_domain(svid.spiffe_id().trust_domain())
        .map_err(|e| anyhow::anyhow!("Failed to get bundle: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("No bundle received"))?;

    workload_api::write_x509_svid_on_update(&svid, &bundle, config)
}
