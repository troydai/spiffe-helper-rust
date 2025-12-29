use clap::{Parser, ValueEnum};

pub const DEFAULT_CONFIG_FILE: &str = "helper.conf";

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum DaemonModeFlag {
    True,
    False,
}

impl DaemonModeFlag {
    pub fn to_bool(self) -> bool {
        match self {
            Self::True => true,
            Self::False => false,
        }
    }
}

/// SPIFFE Helper - A utility for fetching X.509 SVID certificates from the SPIFFE Workload API
#[derive(Parser, Debug)]
#[command(name = "spiffe-helper-rust")]
#[command(about = "SPIFFE Helper - Fetch and manage X.509 SVID certificates", long_about = None)]
pub struct Args {
    /// Path to the configuration file
    #[arg(short, long, default_value = DEFAULT_CONFIG_FILE)]
    pub config: String,

    /// Boolean true or false. Overrides `daemon_mode` in the config file.
    #[arg(long, value_enum)]
    pub daemon_mode: Option<DaemonModeFlag>,

    /// SPIRE agent socket address. Overrides `agent_address` in the config file.
    #[arg(short, long)]
    pub agent_address: Option<String>,

    /// Print version number
    #[arg(short = 'v', long)]
    pub version: bool,
}
