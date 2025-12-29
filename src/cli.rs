use clap::Parser;

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
