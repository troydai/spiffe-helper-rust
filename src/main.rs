use anyhow::Result;
use clap::Parser;

use spiffe_helper_rust::{cli, daemon, svid};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Args::parse();

    match args.get_operation()? {
        cli::Operation::Version => {
            println!("{VERSION}");
            Ok(())
        }
        cli::Operation::RunDaemon(config) => daemon::run(config).await,
        cli::Operation::RunOnce(config) => run_once(config).await,
    }
}

async fn run_once(config: cli::Config) -> Result<()> {
    println!("Running spiffe-helper-rust in one-shot mode...");
    svid::fetch_x509_certificate(&config, config.agent_address()?).await?;
    println!("One-shot mode complete");
    Ok(())
}
