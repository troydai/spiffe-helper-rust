use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

use spiffe_helper_rust::{cli, config, daemon, svid};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Args::parse();

    // Handle version flag
    if args.version {
        println!("{VERSION}");
        return Ok(());
    }

    // Parse daemon_mode override
    let daemon_mode_override = args.daemon_mode.map(cli::DaemonModeFlag::to_bool);

    // Parse config file
    let config_path = PathBuf::from(&args.config);
    let mut config = config::parse_hcl_config(config_path.as_path())
        .with_context(|| format!("Failed to parse config file: {}", args.config))?;

    // Override daemon_mode if provided via CLI
    if let Some(override_value) = daemon_mode_override {
        config.daemon_mode = Some(override_value);
    }

    // Check if daemon mode is enabled (defaults to true)
    let daemon_mode = config.daemon_mode.unwrap_or(true);
    if daemon_mode {
        daemon::run(config).await
    } else {
        run_once(config).await
    }
}

async fn run_once(config: config::Config) -> Result<()> {
    println!("Running spiffe-helper-rust in one-shot mode...");
    svid::fetch_x509_certificate(&config).await?;
    println!("One-shot mode complete");
    Ok(())
}
