pub mod config;

use anyhow::{Context, Result};
use clap::Parser;
pub use config::*;
use std::path::PathBuf;

pub const DEFAULT_CONFIG_FILE: &str = "helper.conf";

/// SPIFFE Helper - A utility for fetching X.509 SVID certificates from the SPIFFE Workload API
#[derive(Parser, Debug)]
#[command(name = "spiffe-helper-rust")]
#[command(about = "SPIFFE Helper - Fetch and manage X.509 SVID certificates", long_about = None)]
pub struct Args {
    /// Path to the configuration file
    #[arg(short, long, default_value = DEFAULT_CONFIG_FILE)]
    pub config: String,

    /// Boolean true or false. Overrides `daemon_mode` in the config file.
    #[arg(long, value_parser = clap::value_parser!(bool), value_name = "BOOL")]
    pub daemon_mode: Option<bool>,

    /// Print version number
    #[arg(short = 'v', long)]
    pub version: bool,
}

pub enum Operation {
    RunDaemon(Config),
    RunOnce(Config),
    Version,
}

impl Args {
    pub fn get_operation(&self) -> Result<Operation> {
        if self.version {
            return Ok(Operation::Version);
        }

        // Parse config file
        let config_path = PathBuf::from(&self.config);
        let mut config = config::parse_hcl_config(config_path.as_path())
            .with_context(|| format!("Failed to parse config file: {}", self.config))?;

        // CLI flag overrides config value (if provided)
        if let Some(daemon_mode) = self.daemon_mode {
            config.daemon_mode = Some(daemon_mode);
        }

        // Check if daemon mode is enabled (defaults to true)
        let daemon_mode = config.daemon_mode.unwrap_or(true);

        // Validate required configuration fields early
        config.validate(daemon_mode)?;

        if daemon_mode {
            Ok(Operation::RunDaemon(config))
        } else {
            Ok(Operation::RunOnce(config))
        }
    }
}
