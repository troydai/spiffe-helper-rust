use anyhow::Result;
use clap::Parser;

use spiffe_helper::{cli, daemon, oneshot};

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
        cli::Operation::RunOnce(config) => oneshot::run(config).await,
    }
}
