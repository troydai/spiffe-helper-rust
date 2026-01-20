use anyhow::{Context, Result};
use spiffe::bundle::BundleSource;
use spiffe::X509Source;
use std::path::Path;
use tokio::process::Command;
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{interval, Duration};

use crate::cli::Config;
use crate::file_system::LocalFileSystem;
use crate::health;
use crate::process;
use crate::signal;
use crate::workload_api;

const DEFAULT_LIVENESS_LOG_INTERVAL_SECS: u64 = 30;

/// Runs the daemon mode: fetches initial certificate, starts health server,
/// and waits for SIGTERM.
pub async fn run(source: X509Source, config: Config) -> Result<()> {
    println!("Starting spiffe-helper daemon...");

    // Parse renew signal if configured
    let renew_signal = config
        .renew_signal
        .as_ref()
        .map(|s| signal::parse_signal_name(s))
        .transpose()
        .context("Failed to parse renew_signal")?;

    println!("Connected to SPIRE agent");

    let local_fs = LocalFileSystem::new(&config)?.ensure()?;

    // Initial fetch and write
    fetch_and_process_update(&source, &local_fs, &config)?;

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

    let mut child_pid = None;
    if let Some(pid) = child.as_ref().and_then(|c| c.id()) {
        match i32::try_from(pid) {
            Ok(pid_i32) => {
                child_pid = Some(pid_i32);
            }
            Err(e) => {
                eprintln!("Failed to convert PID {pid} to i32: {e}");
            }
        }
    }

    let mut health_server = health::HealthCheckServer::new(config.health_checks.as_ref()).await?;

    // Set up signal handling for graceful shutdown
    let mut sigterm =
        signal(SignalKind::terminate()).context("Failed to register SIGTERM handler")?;

    let mut update_channel = source.updated();
    let mut liveness_interval = interval(Duration::from_secs(DEFAULT_LIVENESS_LOG_INTERVAL_SECS));
    liveness_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    println!("Daemon running. Waiting for SIGTERM to shutdown...");

    let mut result: Result<()> = Ok(());

    loop {
        tokio::select! {
            _ = sigterm.recv() => {
                println!("Received SIGTERM, shutting down gracefully...");
                break;
            }
            res = update_channel.changed() => {
                if let Err(e) = res {
                    eprintln!("Update channel closed: {e}");
                    result = Err(anyhow::anyhow!("X509Source update channel closed"));
                    break;
                }

                println!("Received X.509 update notification");
                if let Err(e) = fetch_and_process_update(&source, &local_fs, &config) {
                    eprintln!("Failed to handle X.509 update: {e}");
                    continue;
                }

                send_renew_signal(
                    renew_signal,
                    child_pid,
                    config.pid_file_name.as_deref(),
                );
            }
            res = health_server.wait(), if health_server.is_enabled() => {
                match res {
                    Ok(()) => {
                        println!("Health check server exited unexpectedly");
                    }
                    Err(e) => {
                        eprintln!("Health check server failed: {e}");
                        result = Err(e);
                    }
                }
                break;
            }
            _ = liveness_interval.tick() => {
                println!("spiffe-helper daemon is alive");
            }
            status = async {
                match child.as_mut() {
                    Some(child) => child.wait().await,
                    None => unreachable!(),
                }
            }, if child.is_some() => {
                let status_str = match status {
                    Ok(s) => s.to_string(),
                    Err(e) => format!("error: {e}"),
                };

                child = None;
                child_pid = None;
                println!("Managed process exited: {status_str}");
                // Depending on requirements, we might want to restart it or exit.
                // For now, we'll just stop managing it and continue running the daemon.
            }
        }
    }

    // Shutdown health check server if it was started and still running
    health_server.shutdown();

    if let Some(mut child) = child {
        println!("Stopping managed process...");
        let _ = child.kill().await;
        let _ = child.wait().await;
    }

    println!("Daemon shutdown complete");
    result
}

fn send_renew_signal(
    renew_signal: Option<signal::Signal>,
    child_pid: Option<i32>,
    pid_file: Option<&str>,
) {
    let Some(sig) = renew_signal else {
        return;
    };

    if let Some(pid) = child_pid {
        println!("Sending signal {sig:?} to managed process (PID: {pid})");
        if let Err(e) = signal::send_signal(pid, sig) {
            eprintln!("Failed to signal managed process: {e}");
        }
    }

    if let Some(pid_file) = pid_file {
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

fn fetch_and_process_update(
    source: &X509Source,
    cert_writer: &impl crate::file_system::X509CertsWriter,
    config: &Config,
) -> Result<()> {
    let svid = source
        .svid()
        .map_err(|e| anyhow::anyhow!("Failed to get SVID: {e}"))?;

    let bundle = source
        .bundle_for_trust_domain(svid.spiffe_id().trust_domain())
        .map_err(|e| anyhow::anyhow!("Failed to get bundle: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("No bundle received"))?;

    workload_api::write_x509_svid_on_update(&svid, &bundle, cert_writer, config)
}
