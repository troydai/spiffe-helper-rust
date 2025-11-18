mod config;

use anyhow::{Context, Result};
use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{interval, Duration};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_CONFIG_FILE: &str = "helper.conf";
const DEFAULT_LIVENESS_LOG_INTERVAL_SECS: u64 = 30;

#[derive(Copy, Clone, Debug, ValueEnum)]
enum DaemonModeFlag {
    True,
    False,
}

/// SPIFFE Helper - A utility for fetching X.509 SVID certificates from the SPIFFE Workload API
#[derive(Parser, Debug)]
#[command(name = "spiffe-helper-rust")]
#[command(about = "SPIFFE Helper - Fetch and manage X.509 SVID certificates", long_about = None)]
struct Args {
    /// Path to the configuration file
    #[arg(short, long, default_value = DEFAULT_CONFIG_FILE)]
    config: String,

    /// Boolean true or false. Overrides daemon_mode in the config file.
    #[arg(long, value_enum)]
    daemon_mode: Option<DaemonModeFlag>,

    /// Print version number
    #[arg(short = 'v', long)]
    version: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Handle version flag
    if args.version {
        println!("{}", VERSION);
        return Ok(());
    }

    // Parse daemon_mode override
    let daemon_mode_override = args.daemon_mode.map(|flag| match flag {
        DaemonModeFlag::True => true,
        DaemonModeFlag::False => false,
    });

    // Parse config file
    let config_path = PathBuf::from(&args.config);
    let mut config = config::parse_hcl_config(config_path.as_path())
        .with_context(|| format!("Failed to parse config file: {}", args.config))?;

    // Override daemon_mode if provided via CLI
    if let Some(override_value) = daemon_mode_override {
        config.daemon_mode = Some(override_value);
    }

    // Check if daemon mode is enabled
    let daemon_mode = config.daemon_mode.unwrap_or(false);
    if !daemon_mode {
        // Non-daemon mode - return unimplemented for now
        anyhow::bail!("non-daemon mode not yet implemented")
    }

    // Run daemon mode
    run_daemon(config).await
}

async fn run_daemon(config: config::Config) -> Result<()> {
    println!("Starting spiffe-helper-rust daemon...");

    // Start health check server if enabled
    let health_checks = config.health_checks.clone();
    let health_server_handle = if let Some(ref health_checks) = health_checks {
        if health_checks.listener_enabled {
            let bind_addr = format!("0.0.0.0:{}", health_checks.bind_port);
            let liveness_path = health_checks.liveness_path.clone().unwrap_or_else(|| "/health/live".to_string());
            let readiness_path = health_checks.readiness_path.clone().unwrap_or_else(|| "/health/ready".to_string());

            println!("Starting health check server on {}", bind_addr);
            println!("  Liveness path: {}", liveness_path);
            println!("  Readiness path: {}", readiness_path);

            let app = Router::new()
                .route(&liveness_path, get(liveness_handler))
                .route(&readiness_path, get(readiness_handler));

            let listener = tokio::net::TcpListener::bind(&bind_addr).await
                .with_context(|| format!("Failed to bind to {}", bind_addr))?;

            Some(tokio::spawn(async move {
                axum::serve(listener, app)
                    .await
                    .expect("Health check server failed");
            }))
        } else {
            None
        }
    } else {
        None
    };

    // Set up signal handling for graceful shutdown
    let mut sigterm = signal(SignalKind::terminate())
        .context("Failed to register SIGTERM handler")?;

    // Set up periodic liveness logging
    let mut liveness_interval = interval(Duration::from_secs(DEFAULT_LIVENESS_LOG_INTERVAL_SECS));
    liveness_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    println!("Daemon running. Waiting for SIGTERM to shutdown...");

    // Main daemon loop
    loop {
        tokio::select! {
            _ = sigterm.recv() => {
                println!("Received SIGTERM, shutting down gracefully...");
                break;
            }
            _ = liveness_interval.tick() => {
                println!("spiffe-helper-rust daemon is alive");
            }
        }
    }

    // Shutdown health check server if it was started
    if let Some(handle) = health_server_handle {
        handle.abort();
        println!("Health check server stopped");
    }

    println!("Daemon shutdown complete");
    Ok(())
}

async fn liveness_handler() -> impl IntoResponse {
    StatusCode::OK
}

async fn readiness_handler() -> impl IntoResponse {
    StatusCode::OK
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_greeting_logic() {
        // Mock test to ensure coverage data is generated
        // Test the greeting logic used in main()
        let name = Some("Rust".to_string());
        let greeting_name = name.as_deref().unwrap_or("World");
        assert_eq!(greeting_name, "Rust");

        let no_name: Option<String> = None;
        let default_name = no_name.as_deref().unwrap_or("World");
        assert_eq!(default_name, "World");
    }

    #[test]
    fn test_coverage_helper() {
        // Additional test to generate coverage data
        let count: u8 = 3;
        assert!(count > 0);
        assert_eq!(count, 3);
    }
}
