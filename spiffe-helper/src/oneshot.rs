use crate::{cli::Config, file_system::Storage, workload_api};
use anyhow::Result;
use spiffe::X509Source;

/// Runs the one-shot mode: fetches certificate and exits.
pub async fn run(source: X509Source, config: Config) -> Result<()> {
    println!("Running spiffe-helper in one-shot mode...");
    let cert_dir = config
        .cert_dir
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("cert_dir must be configured"))?;

    let output = Storage::new(&config)?.ensure()?;
    let svid = (*source
        .svid()
        .map_err(|e| anyhow::anyhow!("Failed to fetch X.509 certificate: {e}"))?)
    .clone();

    output.write_certs(svid.cert_chain())?;
    output.write_key(svid.private_key().as_ref())?;

    // Log with SPIFFE ID and certificate expiry (consistent with write_x509_svid_on_update)
    println!(
        "Fetched certificate: spiffe_id={}, expires={}",
        svid.spiffe_id(),
        workload_api::svid_expiry(&svid)
    );

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
