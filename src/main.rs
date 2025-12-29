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

    // Parse config file
    let config_path = PathBuf::from(&args.config);
    let mut config = config::parse_hcl_config(config_path.as_path())
        .with_context(|| format!("Failed to parse config file: {}", args.config))?;

    // CLI flag overrides config value (if provided)
    config.daemon_mode = args.daemon_mode.or(config.daemon_mode);

    // Check if daemon mode is enabled (defaults to true)
    let daemon_mode = config.daemon_mode.unwrap_or(true);

    // Validate agent_address presence
    let agent_address = config
        .agent_address
        .clone()
        .ok_or_else(|| anyhow::anyhow!("agent_address must be configured"))?;

    if daemon_mode {
        daemon::run(config, agent_address).await
    } else {
        run_once(config, agent_address).await
    }
}

async fn run_once(config: config::Config, agent_address: String) -> Result<()> {
    println!("Running spiffe-helper-rust in one-shot mode...");
    svid::fetch_x509_certificate(&config, &agent_address).await?;
    println!("One-shot mode complete");
    Ok(())
}
