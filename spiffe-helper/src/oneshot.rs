use crate::cli::Config;
use crate::workload_api;
use anyhow::{Context, Result};
use spiffe::X509Source;
use std::path::PathBuf;

/// Runs the one-shot mode: fetches certificate and exits.
pub async fn run(source: X509Source, config: Config) -> Result<()> {
    println!("Running spiffe-helper in one-shot mode...");
    let cert_dir = config
        .cert_dir
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("cert_dir must be configured"))?;

    let cert_dir_path = PathBuf::from(cert_dir);
    workload_api::fetch_and_write_x509_svid(
        source,
        &cert_dir_path,
        config.svid_file_name(),
        config.svid_key_file_name(),
    )
    .await
    .with_context(|| "Failed to fetch X.509 certificate")?;

    println!("Successfully fetched and wrote X.509 certificate to {cert_dir}");
    println!("One-shot mode complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_missing_cert_dir() {
        if std::env::var("SPIFFE_ENDPOINT_SOCKET").is_err() {
            return;
        }
        let source = X509Source::new()
            .await
            .expect("Failed to create X509Source for test");
        let config = Config {
            agent_address: Some("unix:///tmp/agent.sock".to_string()),
            cert_dir: None,
            svid_file_name: None,
            svid_key_file_name: None,
            ..Default::default()
        };

        let result = run(source, config).await;
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("cert_dir must be configured"));
    }
}
