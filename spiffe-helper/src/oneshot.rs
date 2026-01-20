use crate::{cli::Config, file_system::Storage};
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

    // write_svid_to_files(
    //     &svid,
    //     &cert_dir_path,
    //     config.svid_file_name(),
    //     config.svid_key_file_name(),
    // )
    // .with_context(|| "Failed to fetch X.509 certificate")?;

    // Log with SPIFFE ID and certificate expiry (consistent with write_x509_svid_on_update)
    let expiry = match x509_parser::parse_x509_certificate(svid.leaf().as_ref()) {
        Ok((_, cert)) => cert
            .validity()
            .not_after
            .to_rfc2822()
            .unwrap_or_else(|_| "unknown".to_string()),
        Err(_) => "unknown".to_string(),
    };
    println!(
        "Fetched certificate: spiffe_id={}, expires={}",
        svid.spiffe_id(),
        expiry
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
