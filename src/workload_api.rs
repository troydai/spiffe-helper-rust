use anyhow::{Context, Result};
use spiffe::workload_api::client::WorkloadApiClient;
use std::fs;
use std::path::Path;

/// Fetches X.509 SVID (certificate and key) from the SPIRE agent
/// and writes them to the specified directory.
///
/// # Arguments
///
/// * `agent_address` - The address of the SPIRE agent (e.g., "<unix:///tmp/agent.sock>")
/// * `cert_dir` - Directory where certificates should be written
/// * `svid_file_name` - Optional filename for the certificate (default: "svid.pem")
/// * `svid_key_file_name` - Optional filename for the private key (default: "`svid_key.pem`")
///
/// # Returns
///
/// Returns `Ok(())` if successful, or an error if fetching or writing fails.
pub async fn fetch_and_write_x509_svid(
    agent_address: &str,
    cert_dir: &Path,
    svid_file_name: Option<&str>,
    svid_key_file_name: Option<&str>,
) -> Result<()> {
    // Create cert directory if it doesn't exist
    fs::create_dir_all(cert_dir)
        .with_context(|| format!("Failed to create cert directory: {}", cert_dir.display()))?;

    // Create Workload API client
    // Handle unix:// URLs by using new_from_path, otherwise convert to unix: format
    let mut client = if agent_address.starts_with("unix://") {
        let socket_path = agent_address
            .strip_prefix("unix://")
            .ok_or_else(|| anyhow::anyhow!("Invalid unix socket address: {agent_address}"))?;
        // Use unix: prefix (not unix://) for spiffe crate
        let spiffe_path = format!("unix:{socket_path}");
        WorkloadApiClient::new_from_path(&spiffe_path)
            .await
            .with_context(|| format!("Failed to connect to SPIRE agent at {agent_address}"))?
    } else {
        // For non-unix addresses, try new_from_path with the address as-is
        // If it's already a valid address format, new_from_path should handle it
        WorkloadApiClient::new_from_path(agent_address)
            .await
            .with_context(|| format!("Failed to connect to SPIRE agent at {agent_address}"))?
    };

    // Fetch X.509 SVID with retries (workload may need time to attest)
    let mut svid = None;
    let mut last_error_msg = None;
    for attempt in 1..=10 {
        match client.fetch_x509_svid().await {
            Ok(s) => {
                svid = Some(s);
                if attempt > 1 {
                    eprintln!("Successfully fetched X.509 SVID after {attempt} attempts");
                }
                break;
            }
            Err(e) => {
                let error_str = format!("{e:?}");
                last_error_msg = Some(format!("{e} ({error_str})"));
                // If it's a permission denied error, the workload may still be attesting
                if error_str.contains("PermissionDenied") && attempt < 10 {
                    // Wait before retrying (exponential backoff: 1s, 2s, 4s, 8s, 16s, etc., max 16s)
                    let delay = std::cmp::min(1u64 << (attempt - 1), 16);
                    eprintln!(
                        "Attempt {attempt} failed (PermissionDenied), retrying in {delay}s..."
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                    continue;
                }
                // For other errors or last attempt, return the error
                return Err(anyhow::anyhow!(
                    "Failed to fetch X.509 SVID from SPIRE agent after {} attempts: {}",
                    attempt,
                    last_error_msg.as_ref().unwrap_or(&format!("{e:?}"))
                ));
            }
        }
    }

    let svid = svid.ok_or_else(|| {
        anyhow::anyhow!(
            "Failed to fetch X.509 SVID from SPIRE agent: {}",
            last_error_msg.unwrap_or_else(|| "unknown error".to_string())
        )
    })?;

    // Determine file paths
    let cert_file_name = svid_file_name.unwrap_or("svid.pem");
    let key_file_name = svid_key_file_name.unwrap_or("svid_key.pem");
    let cert_path = cert_dir.join(cert_file_name);
    let key_path = cert_dir.join(key_file_name);

    // Write certificate (PEM format)
    let cert_pem = svid
        .cert_chain()
        .iter()
        .map(|cert| {
            pem::encode(&pem::Pem {
                tag: "CERTIFICATE".to_string(),
                contents: cert.as_ref().to_vec(),
            })
        })
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(&cert_path, cert_pem)
        .with_context(|| format!("Failed to write certificate to {}", cert_path.display()))?;

    // Write private key (PEM format)
    let key_pem = pem::encode(&pem::Pem {
        tag: "PRIVATE KEY".to_string(),
        contents: svid.private_key().as_ref().to_vec(),
    });

    fs::write(&key_path, key_pem)
        .with_context(|| format!("Failed to write private key to {}", key_path.display()))?;

    Ok(())
}
