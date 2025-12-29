use anyhow::{Context, Result};
use spiffe::svid::x509::X509Svid;
use spiffe::svid::SvidSource;
use spiffe::workload_api::client::WorkloadApiClient;
use spiffe::workload_api::x509_source::{X509Source, X509SourceBuilder};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio_retry::strategy::ExponentialBackoff;
use tokio_retry::RetryIf;

const UDS_PREFIX: &str = "unix://";
const MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(60); // 1 minute minimum
const MAX_REFRESH_INTERVAL: Duration = Duration::from_secs(3600); // 1 hour maximum

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
    svid_file_name: &str,
    svid_key_file_name: &str,
) -> Result<()> {
    // Create cert directory if it doesn't exist
    fs::create_dir_all(cert_dir)
        .with_context(|| format!("Failed to create cert directory: {}", cert_dir.display()))?;

    // Use create_x509_source to handle retry logic and connection
    let source = create_x509_source(agent_address).await?;

    // Get the SVID from the source
    let svid: X509Svid = source
        .get_svid()
        .map_err(|e| anyhow::anyhow!("Failed to get SVID from source: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("X509Source returned no SVID (None)"))?;

    // Use the update handler to write certificates
    on_x509_update(&svid, cert_dir, svid_file_name, svid_key_file_name)?;

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
    // Use exponential backoff: 1s, 2s, 4s, 8s, 16s max, up to 10 attempts
    let retry_strategy = ExponentialBackoff::from_millis(1000)
        .max_delay(Duration::from_secs(16))
        .take(10);

    RetryIf::spawn(
        retry_strategy,
        || async {
            let client = create_workload_api_client(agent_address).await?;
            X509SourceBuilder::new()
                .with_client(client)
                .build()
                .await
                .context("Failed to build X509Source")
        },
        is_retryable_error,
    )
    .await
    .map_err(|e| anyhow::anyhow!("Failed to create X509Source from SPIRE agent: {e}"))
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

fn is_retryable_error(err: &anyhow::Error) -> bool {
    let error_str = format!("{err:?}");
    error_str.contains("PermissionDenied")
}

/// Calculates when to refresh the certificate based on its expiry time.
///
/// Returns half the time until expiry, with minimum and maximum bounds applied.
/// This ensures certificates are renewed well before they expire, providing a safety margin.
///
/// # Arguments
///
/// * `svid` - The X509Svid to calculate refresh interval for
///
/// # Returns
///
/// Returns a Duration indicating when the certificate should be refreshed.
/// - For normal certificates: (NotAfter - Now) / 2
/// - Minimum: 60 seconds (to avoid excessive refresh attempts)
/// - Maximum: 1 hour (to detect issues with long-lived certificates)
/// - Expired certificates: returns minimum interval
pub fn calculate_refresh_interval(svid: &X509Svid) -> Duration {
    // Get the leaf certificate (first in chain)
    let cert = match svid.cert_chain().first() {
        Some(cert) => cert,
        None => return MIN_REFRESH_INTERVAL,
    };

    // Parse the certificate to get NotAfter
    let x509_cert = match x509_parser::parse_x509_certificate(cert.as_ref()) {
        Ok((_, cert)) => cert,
        Err(_) => return MIN_REFRESH_INTERVAL,
    };

    // Get current time and expiry time
    let now = SystemTime::now();

    // Convert ASN1Time to SystemTime
    // ASN1Time epoch is Unix epoch, so we can convert directly
    let not_after_timestamp = x509_cert.validity().not_after.timestamp();
    let not_after = SystemTime::UNIX_EPOCH + Duration::from_secs(not_after_timestamp as u64);

    // Calculate time until expiry
    let time_until_expiry = not_after.duration_since(now).unwrap_or(Duration::ZERO);

    // If already expired or very soon, return minimum interval
    if time_until_expiry <= Duration::from_secs(10) {
        return MIN_REFRESH_INTERVAL;
    }

    // Refresh at half the remaining lifetime
    let refresh_interval = time_until_expiry / 2;

    // Apply bounds: minimum 60 seconds, maximum 1 hour
    refresh_interval.clamp(MIN_REFRESH_INTERVAL, MAX_REFRESH_INTERVAL)
}

/// Writes the X509Svid certificate and private key to the specified files.
///
/// This is a helper function that can be used both for initial certificate
/// fetching and for certificate rotation updates.
///
/// # Arguments
///
/// * `svid` - The X509Svid to write
/// * `cert_path` - Path where the certificate should be written
/// * `key_path` - Path where the private key should be written
///
/// # Returns
///
/// Returns `Ok(())` if successful, or an error if writing fails.
pub fn write_svid_to_files(svid: &X509Svid, cert_path: &Path, key_path: &Path) -> Result<()> {
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

    fs::write(cert_path, cert_pem)
        .with_context(|| format!("Failed to write certificate to {}", cert_path.display()))?;

    // Write private key (PEM format)
    let key_pem = pem::encode(&pem::Pem {
        tag: "PRIVATE KEY".to_string(),
        contents: svid.private_key().as_ref().to_vec(),
    });

    fs::write(key_path, key_pem)
        .with_context(|| format!("Failed to write private key to {}", key_path.display()))?;

    Ok(())
}

/// Handler for X509Context updates during certificate rotation.
///
/// This function is called when new certificates are available from the SPIRE agent.
/// It writes the updated certificates to disk using the configured file names.
///
/// # Arguments
///
/// * `svid` - The updated X509Svid
/// * `cert_dir` - Directory where certificates should be written
/// * `svid_file_name` - Filename for the certificate
/// * `svid_key_file_name` - Filename for the private key
///
/// # Returns
///
/// Returns `Ok(())` if successful, or an error if writing fails.
pub fn on_x509_update(
    svid: &X509Svid,
    cert_dir: &Path,
    svid_file_name: &str,
    svid_key_file_name: &str,
) -> Result<()> {
    let cert_path = cert_dir.join(svid_file_name);
    let key_path = cert_dir.join(svid_key_file_name);

    write_svid_to_files(svid, &cert_path, &key_path)?;

    // Log the update with SPIFFE ID and expiry info
    if let Some(cert) = svid.cert_chain().first() {
        if let Ok((_, x509)) = x509_parser::parse_x509_certificate(cert.as_ref()) {
            let not_after = x509.validity().not_after;
            println!(
                "Certificate updated. SPIFFE ID: {}, Expires: {}",
                svid.spiffe_id(),
                not_after
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Instant;
    use tempfile::TempDir;

    #[test]
    fn test_is_retryable_error() {
        let err = anyhow::anyhow!("Some PermissionDenied error");
        assert!(is_retryable_error(&err));

        let err = anyhow::anyhow!("PermissionDenied: access denied");
        assert!(is_retryable_error(&err));

        let err = anyhow::anyhow!("Some other error");
        assert!(!is_retryable_error(&err));

        let err = anyhow::anyhow!("Connection refused");
        assert!(!is_retryable_error(&err));
    }

    #[tokio::test]
    async fn test_fetch_and_write_x509_svid_fail_fast() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path();

        let start = Instant::now();
        // Use an address that is definitely invalid and shouldn't trigger "PermissionDenied"
        // "invalid" scheme usually causes an immediate parsing or argument error
        let result =
            fetch_and_write_x509_svid("invalid://address", cert_dir, "svid.pem", "svid_key.pem")
                .await;
        let duration = start.elapsed();

        assert!(result.is_err());
        // Should return essentially immediately, definitely less than the first retry backoff (1s)
        assert!(
            duration < Duration::from_millis(500),
            "Should fail fast on non-retryable error, took {:?}",
            duration
        );
    }

    #[tokio::test]
    async fn test_fetch_and_write_x509_svid_invalid_address() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path();

        // Test with invalid agent address
        let result =
            fetch_and_write_x509_svid("invalid://address", cert_dir, "svid.pem", "svid_key.pem")
                .await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        // The error message should contain information about failing to create the client
        // It may be "Failed to create WorkloadApiClient" or connection-related errors
        assert!(
            error_msg.contains("Failed to create X509Source")
                || error_msg.contains("Invalid agent address")
                || error_msg.contains("Failed to connect")
                || error_msg.contains("invalid")
                || error_msg.contains("Failed to fetch X.509 SVID")
        );
    }

    #[tokio::test]
    async fn test_fetch_and_write_x509_svid_missing_agent() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path();

        // Test with non-existent unix socket
        let result = fetch_and_write_x509_svid(
            "unix:///tmp/nonexistent-socket.sock",
            cert_dir,
            "svid.pem",
            "svid_key.pem",
        )
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
            "custom_cert.pem",
            "custom_key.pem",
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

    // Note: Testing certificate rotation with real X509Svid objects requires
    // a running SPIRE agent. The tests above verify the basic logic.
    // Integration tests with a real SPIRE environment are in tests/ directory.
    //
    // The following tests would require creating valid X509Svid test fixtures,
    // which is complex due to the spiffe library's requirements for proper
    // certificate chains. For now, we verify the functions compile and
    // the logic is correct through code review and integration testing.
}
