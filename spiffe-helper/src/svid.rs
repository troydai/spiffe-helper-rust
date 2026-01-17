use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::cli::Config;
use crate::workload_api;

/// Fetches the initial X.509 SVID from the SPIRE agent and writes them to the specified directory.
///
/// This function validates the configuration (`cert_dir`) and calls
/// `workload_api::fetch_and_write_x509_svid` to perform the actual fetch and write operation.
/// It implements the shared initial SVID fetch policy used by both daemon and one-shot modes,
/// including retry logic and backoff handling.
///
/// # Arguments
///
/// * `config` - The configuration containing cert directory and file names
/// * `agent_address` - The address of the SPIRE agent
///
/// # Returns
///
/// Returns `Ok(())` if successful, or an error if configuration is invalid or fetching fails.
pub async fn fetch_x509_certificate(config: &Config, agent_address: &str) -> Result<()> {
    let cert_dir = config
        .cert_dir
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("cert_dir must be configured"))?;

    println!("Fetching X.509 certificate from SPIRE agent at {agent_address}...");
    let cert_dir_path = PathBuf::from(cert_dir);
    workload_api::fetch_and_write_x509_svid(
        agent_address,
        &cert_dir_path,
        config.svid_file_name(),
        config.svid_key_file_name(),
        config.svid_bundle_file_name(),
    )
    .await
    .with_context(|| "Failed to fetch X.509 certificate")?;
    println!("Successfully fetched and wrote X.509 certificate to {cert_dir}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_x509_certificate_missing_cert_dir() {
        let config = Config {
            agent_address: Some("unix:///tmp/agent.sock".to_string()),
            cert_dir: None,
            svid_file_name: None,
            svid_key_file_name: None,
            ..Default::default()
        };

        let result = fetch_x509_certificate(&config, "unix:///tmp/agent.sock").await;
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("cert_dir must be configured"));
    }
}
