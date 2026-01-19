use anyhow::{anyhow, Result};
use clap::Parser;

use spiffe_helper::{cli, daemon, oneshot, workload_api};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Args::parse();

    if args.is_version_op() {
        println!("{VERSION}");
        return Ok(());
    }

    let config = args.get_operation_config()?;
    let x509_source = workload_api::create_x509_source(
        config
            .agent_address
            .as_ref()
            .ok_or_else(|| anyhow!("missing agent address"))?,
    )
    .await?;

    if !config.is_daemon_mode() {
        return oneshot::run(x509_source, config).await;
    }

    daemon::run(config).await
}
