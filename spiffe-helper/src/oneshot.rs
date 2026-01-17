use crate::cli::Config;
use crate::svid;
use anyhow::Result;

/// Runs the one-shot mode: fetches certificate and exits.
pub async fn run(config: Config) -> Result<()> {
    println!("Running spiffe-helper in one-shot mode...");
    svid::fetch_x509_certificate(&config, config.agent_address()?).await?;
    println!("One-shot mode complete");
    Ok(())
}
