use crate::{cli::Config, file_system::LocalFileSystem, workload_api};
use anyhow::Result;
use spiffe::bundle::BundleSource;
use spiffe::X509Source;

/// Runs the one-shot mode: fetches certificate and exits.
pub async fn run(source: X509Source, config: Config) -> Result<()> {
    println!("Running spiffe-helper in one-shot mode...");
    let cert_dir = config
        .cert_dir
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("cert_dir must be configured"))?;

    let local_fs = LocalFileSystem::new(&config)?.ensure()?;
    let svid = (*source
        .svid()
        .map_err(|e| anyhow::anyhow!("Failed to fetch X.509 certificate: {e}"))?)
    .clone();
    let bundle = source
        .bundle_for_trust_domain(svid.spiffe_id().trust_domain())
        .map_err(|e| anyhow::anyhow!("Failed to get bundle: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("No bundle received"))?;

    workload_api::write_x509_svid_on_update(&svid, &bundle, &local_fs, &config)?;

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
