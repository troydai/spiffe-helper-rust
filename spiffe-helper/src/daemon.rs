use anyhow::{Context, Result};
use spiffe::bundle::BundleSource;
use spiffe::X509Source;
use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{interval, Duration};
use tokio_util::sync::CancellationToken;

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
    let source = workload_api::create_x509_source(config.agent_address()?).await?;
    println!("Connected to SPIRE agent");

    // Initial fetch and write
    fetch_and_process_update(&source, &config)?;

    // Spawn managed child process if configured
    let child = if let Some(cmd) = &config.cmd {
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

    let child_pid = Arc::new(AtomicI32::new(0));
    if let Some(pid) = child.as_ref().and_then(|c| c.id()) {
        match i32::try_from(pid) {
            Ok(pid_i32) => {
                child_pid.store(pid_i32, Ordering::Relaxed);
            }
            Err(e) => {
                eprintln!("Failed to convert PID {pid} to i32: {e}");
            }
        }
    }

    let shutdown = CancellationToken::new();

    let cert_task = tokio::spawn(cert_update_worker(
        Arc::new(source),
        config.clone(),
        renew_signal,
        child_pid.clone(),
        shutdown.clone(),
    ));

    let liveness_task = tokio::spawn(liveness_worker(shutdown.clone()));

    let child_task =
        child.map(|c| tokio::spawn(child_monitor_worker(c, child_pid.clone(), shutdown.clone())));

    println!("Daemon running. Waiting for SIGTERM to shutdown...");

    let mut result: Result<()> = Ok(());

    tokio::select! {
        _ = sigterm.recv() => {
            println!("Received SIGTERM, shutting down gracefully...");
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
        }
        _ = shutdown.cancelled() => {
            println!("Shutdown requested");
        }
    }

    shutdown.cancel();

    if let Err(e) = liveness_task.await {
        eprintln!("Liveness worker task error: {e}");
    }

    if let Some(task) = child_task {
        if let Err(e) = task.await {
            eprintln!("Child monitor worker task error: {e}");
        }
    }

    match cert_task.await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            if result.is_ok() {
                result = Err(e);
            }
        }
        Err(e) => {
            if result.is_ok() {
                result = Err(anyhow::anyhow!("Certificate worker task error: {e}"));
            }
        }
    }

    // Shutdown health check server if it was started and still running
    health_server.shutdown();

    println!("Daemon shutdown complete");
    result
}

async fn cert_update_worker(
    source: Arc<X509Source>,
    config: Config,
    renew_signal: Option<signal::Signal>,
    child_pid: Arc<AtomicI32>,
    shutdown: CancellationToken,
) -> Result<()> {
    let mut update_channel = source.updated();

    loop {
        tokio::select! {
            biased;

            _ = shutdown.cancelled() => {
                break Ok(());
            }
            res = update_channel.changed() => {
                if let Err(e) = res {
                    eprintln!("Update channel closed: {e}");
                    shutdown.cancel();
                    break Err(anyhow::anyhow!("X509Source update channel closed"));
                }

                println!("Received X.509 update notification");
                if let Err(e) = fetch_and_process_update(&source, &config) {
                    eprintln!("Failed to handle X.509 update: {e}");
                    continue;
                }

                if let Some(sig) = renew_signal {
                    let pid = child_pid.load(Ordering::Relaxed);
                    if pid > 0 {
                        println!("Sending signal {sig:?} to managed process (PID: {pid})");
                        if let Err(e) = signal::send_signal(pid, sig) {
                            eprintln!("Failed to signal managed process: {e}");
                        }
                    }

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
    }
}

async fn liveness_worker(shutdown: CancellationToken) {
    let mut liveness_interval = interval(Duration::from_secs(DEFAULT_LIVENESS_LOG_INTERVAL_SECS));
    liveness_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                break;
            }
            _ = liveness_interval.tick() => {
                println!("spiffe-helper daemon is alive");
            }
        }
    }
}

async fn child_monitor_worker(
    mut child: Child,
    child_pid: Arc<AtomicI32>,
    shutdown: CancellationToken,
) {
    let status = tokio::select! {
        res = child.wait() => res,
        _ = shutdown.cancelled() => {
            println!("Stopping managed process...");
            let _ = child.kill().await;
            let _ = child.wait().await;
            child_pid.store(0, Ordering::Relaxed);
            return;
        }
    };

    let status_str = match status {
        Ok(s) => s.to_string(),
        Err(e) => format!("error: {e}"),
    };

    child_pid.store(0, Ordering::Relaxed);
    println!("Managed process exited: {status_str}");
    // Depending on requirements, we might want to restart it or exit.
    // For now, we'll just stop managing it and continue running the daemon.
}

fn fetch_and_process_update(source: &X509Source, config: &Config) -> Result<()> {
    let svid = source
        .svid()
        .map_err(|e| anyhow::anyhow!("Failed to get SVID: {e}"))?;

    let bundle = source
        .bundle_for_trust_domain(svid.spiffe_id().trust_domain())
        .map_err(|e| anyhow::anyhow!("Failed to get bundle: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("No bundle received"))?;

    workload_api::write_x509_svid_on_update(&svid, &bundle, config)
}
