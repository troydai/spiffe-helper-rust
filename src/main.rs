mod config;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_CONFIG_FILE: &str = "helper.conf";

/// SPIFFE Helper - A utility for fetching X.509 SVID certificates from the SPIFFE Workload API
#[derive(Parser, Debug)]
#[command(name = "spiffe-helper-rust")]
#[command(about = "SPIFFE Helper - Fetch and manage X.509 SVID certificates", long_about = None)]
struct Args {
    /// Path to the configuration file
    #[arg(short, long, default_value = DEFAULT_CONFIG_FILE)]
    config: String,

    /// Boolean true or false. Overrides daemon_mode in the config file.
    #[arg(long, value_name = "true|false")]
    daemon_mode: Option<String>,

    /// Print version number
    #[arg(short = 'v', long)]
    version: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Handle version flag
    if args.version {
        println!("{}", VERSION);
        return Ok(());
    }

    // Parse daemon_mode override
    let daemon_mode_override = if let Some(dm_str) = &args.daemon_mode {
        match dm_str.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => {
                anyhow::bail!(
                    "Invalid value for -daemon-mode: {}. Must be 'true' or 'false'",
                    dm_str
                );
            }
        }
    } else {
        None
    };

    // Parse config file
    let config_path = PathBuf::from(&args.config);
    let mut config = config::parse_hcl_config(config_path.as_path())
        .with_context(|| format!("Failed to parse config file: {}", args.config))?;

    // Override daemon_mode if provided via CLI
    if let Some(override_value) = daemon_mode_override {
        config.daemon_mode = Some(override_value);
    }

    // TODO: Implement actual functionality
    // For now, return unimplemented error
    anyhow::bail!("unimplemented")
}
