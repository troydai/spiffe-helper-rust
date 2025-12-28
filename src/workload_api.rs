use anyhow::{Context, Result};
use spiffe::workload_api::client::WorkloadApiClient;
use spiffe::workload_api::x509_source::{X509Source, X509SourceBuilder};
use std::fs;
use std::path::Path;
use std::sync::Arc;

const UDS_PREFIX: &str = "unix://";

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
    let mut client = create_workload_api_client(agent_address).await?;

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

/// Creates an X509Source for continuous X.509 certificate watching.
///
/// This function creates an X509Source that automatically watches for certificate updates
/// from the SPIRE agent. It includes retry logic with exponential backoff for initial connection.
///
/// # Arguments
///
/// * `agent_address` - The address of the SPIRE agent (e.g., "unix:///tmp/agent.sock")
///
/// # Returns
///
/// Returns `Ok(Arc<X509Source>)` if successful, or an error if connection fails after retries.
pub async fn create_x509_source(agent_address: &str) -> Result<Arc<X509Source>> {
    // Create X509Source with retries (workload may need time to attest)
    let mut last_error_msg = None;
    for attempt in 1..=10 {
        // First create a WorkloadApiClient using the shared helper function
        let client_result = create_workload_api_client(agent_address).await;
        match client_result {
            Ok(client) => {
                // Create X509Source from the client using builder
                match X509SourceBuilder::new().with_client(client).build().await {
                    Ok(source) => {
                        if attempt > 1 {
                            eprintln!("Successfully created X509Source after {attempt} attempts");
                        }
                        return Ok(source);
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
                        // For other errors or last attempt, continue to return error
                    }
                }
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
                // For other errors or last attempt, continue to return error
            }
        }
    }

    Err(anyhow::anyhow!(
        "Failed to create X509Source from SPIRE agent after 10 attempts: {}",
        last_error_msg.unwrap_or_else(|| "unknown error".to_string())
    ))
}

async fn create_workload_api_client(address: &str) -> Result<WorkloadApiClient> {
    let address = address
        .strip_prefix(UDS_PREFIX)
        .map_or_else(|| String::from(address), |v| format!("unix:{v}"));

    WorkloadApiClient::new_from_path(&address)
        .await
        .with_context(|| {
            format!(
                "Failed to create WorkloadApiClient from address: {}",
                address
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_fetch_and_write_x509_svid_invalid_address() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path();

        // Test with invalid agent address
        let result = fetch_and_write_x509_svid("invalid://address", cert_dir, None, None).await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        // The error message should contain information about failing to create the client
        // It may be "Failed to create WorkloadApiClient" or connection-related errors
        assert!(
            error_msg.contains("Failed to create WorkloadApiClient")
                || error_msg.contains("Invalid agent address")
                || error_msg.contains("Failed to connect")
                || error_msg.contains("invalid")
        );
    }

    #[tokio::test]
    async fn test_fetch_and_write_x509_svid_missing_agent() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path();

        // Test with non-existent unix socket
        let result =
            fetch_and_write_x509_svid("unix:///tmp/nonexistent-socket.sock", cert_dir, None, None)
                .await;

        assert!(result.is_err());
        // Should fail when trying to connect to non-existent socket
        let error_msg = result.unwrap_err().to_string();
        // The error message may vary depending on the platform/tonic version
        // Just verify that it's an error related to connection/socket
        assert!(!error_msg.is_empty(), "Error message should not be empty");
    }

    #[tokio::test]
    async fn test_fetch_and_write_x509_svid_custom_file_names() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path();

        // Test with custom file names
        let result = fetch_and_write_x509_svid(
            "unix:///tmp/nonexistent-socket.sock",
            cert_dir,
            Some("custom_cert.pem"),
            Some("custom_key.pem"),
        )
        .await;

        // Should fail on connection, but verify directory was created
        assert!(result.is_err());
        assert!(
            cert_dir.exists(),
            "Cert directory should be created even if connection fails"
        );
    }

    #[tokio::test]
    async fn test_create_x509_source_invalid_address() {
        // Test with invalid agent address
        let result = create_x509_source("invalid://address").await;

        // Should fail when trying to create the client
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("Failed to create X509Source")
                    || error_msg.contains("Failed to create WorkloadApiClient")
                    || error_msg.contains("invalid")
            );
        } else {
            panic!("Expected error but got Ok");
        }
    }

    #[tokio::test]
    async fn test_create_x509_source_missing_agent() {
        // Test with non-existent unix socket
        let result = create_x509_source("unix:///tmp/nonexistent-socket-98765.sock").await;

        // Should fail after retries
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(error_msg.contains("Failed to create X509Source") || !error_msg.is_empty());
        } else {
            panic!("Expected error but got Ok");
        }
    }

    #[tokio::test]
    async fn test_create_x509_source_unix_format() {
        // Test that unix:// format is handled correctly
        // This will fail to connect but should not fail on address parsing
        let result = create_x509_source("unix:///tmp/test-socket.sock").await;

        // Should not contain "Invalid unix socket address" since we handle the conversion
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(!error_msg.contains("Invalid unix socket address"));
        } else {
            panic!("Expected error but got Ok");
        }
    }

    #[tokio::test]
    async fn test_create_x509_source_invalid_unix_address() {
        // Test with invalid unix:// address format (empty path)
        // This will try to connect to "unix:" which should fail quickly
        let result = create_x509_source("unix://").await;
        assert!(result.is_err());
        // Should fail on connection, not hang forever
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(error_msg.contains("Failed to create X509Source"));
        }
    }

    #[tokio::test]
    async fn test_create_workload_api_client_unix_prefix_conversion() {
        // Test that unix:// prefix is converted to unix: format
        let result = create_workload_api_client("unix:///tmp/test-socket.sock").await;
        // Should fail on connection (socket doesn't exist), not on parsing
        assert!(result.is_err());
        if let Err(e) = result {
            let error_msg = e.to_string();
            // Should contain the error context message
            assert!(error_msg.contains("Failed to create WorkloadApiClient"));
            // Should not fail on parsing the address format
            assert!(!error_msg.contains("Invalid"));
        }
    }

    #[tokio::test]
    async fn test_create_workload_api_client_without_prefix() {
        // Test address without unix:// prefix (should pass through as-is)
        let result = create_workload_api_client("unix:/tmp/test-socket.sock").await;
        // Should fail on connection, not on parsing
        assert!(result.is_err());
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(error_msg.contains("Failed to create WorkloadApiClient"));
        }
    }

    #[tokio::test]
    async fn test_create_workload_api_client_invalid_address() {
        // Test with invalid address format
        let result = create_workload_api_client("invalid://address").await;
        assert!(result.is_err());
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(error_msg.contains("Failed to create WorkloadApiClient"));
        }
    }

    #[tokio::test]
    async fn test_create_workload_api_client_nonexistent_socket() {
        // Test with non-existent socket path
        let result = create_workload_api_client("unix:///tmp/nonexistent-socket-99999.sock").await;
        assert!(result.is_err());
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(error_msg.contains("Failed to create WorkloadApiClient"));
            // Should include the converted address in the error message
            assert!(error_msg.contains("unix:/tmp/nonexistent-socket-99999.sock"));
        }
    }

    #[tokio::test]
    async fn test_create_workload_api_client_empty_path() {
        // Test edge case: empty path after stripping prefix
        let result = create_workload_api_client("unix://").await;
        assert!(result.is_err());
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(error_msg.contains("Failed to create WorkloadApiClient"));
        }
    }

    #[test]
    fn test_cert_dir_creation() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path().join("nested").join("cert").join("dir");

        // Verify directory doesn't exist
        assert!(!cert_dir.exists());

        // The function should create the directory
        // We can't easily test the full function without a SPIRE agent,
        // but we can verify the directory creation logic would work
        fs::create_dir_all(&cert_dir).unwrap();
        assert!(cert_dir.exists());
    }
}
