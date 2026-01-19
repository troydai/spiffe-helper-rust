use crate::cli::config::{self, Config};
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::path::PathBuf;

pub const DEFAULT_CONFIG_FILE: &str = "helper.conf";

/// SPIFFE Helper - A utility for fetching X.509 SVID certificates from the SPIFFE Workload API
#[derive(Parser, Debug)]
#[command(name = "spiffe-helper")]
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

impl Args {
    pub fn get_operation_config(&self) -> Result<Config> {
        if self.version {
            return Err(anyhow!("Unexpected error: should return version"));
        }

        // Parse config file
        let config_path = PathBuf::from(&self.config);
        let mut config = config::parse_hcl_config(config_path.as_path())
            .with_context(|| format!("Failed to parse config file: {}", self.config))?;

        // Merge CLI flag with config value and default to true
        config.reconcile_daemon_mode(self.daemon_mode);

        // Validate required configuration fields early
        config.validate()?;

        Ok(config)
    }
}
