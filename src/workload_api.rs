use anyhow::{Context, Result};
use crate::config::Config;
use spiffe::bundle::x509::X509Bundle;
use spiffe::svid::x509::X509Svid;
use spiffe::svid::SvidSource;
use spiffe::workload_api::client::WorkloadApiClient;
use spiffe::workload_api::x509_source::{X509Source, X509SourceBuilder};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio_retry::strategy::ExponentialBackoff;
use tokio_retry::RetryIf;

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

    write_svid_to_files(&svid, cert_dir, svid_file_name, svid_key_file_name).await
}

/// Writes X.509 SVID (certificate and key) to the specified directory.
pub async fn write_svid_to_files(
    svid: &X509Svid,
    cert_dir: &Path,
    svid_file_name: &str,
    svid_key_file_name: &str,
) -> Result<()> {
    // Create cert directory if it doesn't exist
    fs::create_dir_all(cert_dir)
        .with_context(|| format!("Failed to create cert directory: {}", cert_dir.display()))?;

    let cert_path = cert_dir.join(svid_file_name);
    let key_path = cert_dir.join(svid_key_file_name);

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

/// Handler for X509Context updates
pub async fn on_x509_update(
    svid: &X509Svid,
    _bundle: &X509Bundle,
    config: &Config,
) -> Result<()> {
    if let Some(cert_dir) = &config.cert_dir {
        let cert_dir = Path::new(cert_dir);
        let svid_file_name = config.svid_file_name();
        let svid_key_file_name = config.svid_key_file_name();

        write_svid_to_files(svid, cert_dir, svid_file_name, svid_key_file_name).await?;
        
        // Future: write bundle
        // Future: signal process
    }
    
    // Log the update
    eprintln!("Updated X.509 SVID: {}", svid.spiffe_id());
    
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

    #[tokio::test]
    async fn test_write_svid_to_files() {
        use spiffe::svid::x509::X509Svid;

        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path();
        
        // Self-signed cert (with SAN) and key for testing
        let cert_pem = r#"-----BEGIN CERTIFICATE-----
MIIDsjCCApqgAwIBAgIUdSDbCP50csvka2qEZDl7ZzdTTfswDQYJKoZIhvcNAQEL
BQAwTjELMAkGA1UEBhMCVVMxDjAMBgNVBAgMBVN0YXRlMQ0wCwYDVQQHDARDaXR5
MQwwCgYDVQQKDANPcmcxEjAQBgNVBAMMCWxvY2FsaG9zdDAeFw0yNTEyMjkwMDI5
MjNaFw0yNjEyMjkwMDI5MjNaME4xCzAJBgNVBAYTAlVTMQ4wDAYDVQQIDAVTdGF0
ZTENMAsGA1UEBwwEQ2l0eTEMMAoGA1UECgwDT3JnMRIwEAYDVQQDDAlsb2NhbGhv
c3QwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQDBQIhUXd1gXpuLtHlB
8tXDGYQ63sdlvnWrCOxfKBW1yhNUEDQbF0e+Ij/wrHMiyY9P6I2cxty6LxFCp33g
jUeUJhV+2eEkHxsWBP6AnJ2x30i5LmkDQNDr7fVtMAjckHUN16I3f3Q5ntJ2E55U
VR1Tx7bNTKKfd0zRt3AaQF4OKiVROggxriFZxlWlKW4CyXVzv7bLKwuEYSuwgAFw
raM3YfRJZDd+IhJDg7Sku9HBQoVzAh8LdlTdoaZd5AEUrz+PXfw5T+Guqm4T1ckW
BLiHKsShZkNIGiQDIhN/GJpJ2BwymdMi0fc3Krlu2sqtZXOiMsuyjbIDoIDlEWEE
L6ulAgMBAAGjgYcwgYQwDgYDVR0PAQH/BAQDAgWgMB0GA1UdJQQWMBQGCCsGAQUF
BwMBBggrBgEFBQcDAjAJBgNVHRMEAjAAMCkGA1UdEQQiMCCGHnNwaWZmZTovL2V4
YW1wbGUub3JnL215c2VydmljZTAdBgNVHQ4EFgQUqCNQRmkVxf7dHBK13TUWnAkE
CGMwDQYJKoZIhvcNAQELBQADggEBAFWq3lRoRIFfS7SvnOIVBq0o3xBkN9umxgHE
PcjEPe9O1PWrEMFAtikQCVDsWE89YIwRfMUoctleO4wD/WgtehQUDnnGPOLTkfi2
+gpLm3j/lRw4MuCGNgT04XQqeD/RWkVMfrTri/dDXruKEAat97T4AgfChwIhSgWE
YcKShkPHnnQjrPoItWhUbJpixcJ3MUbdgC3X958V062+g2HsiuTRZzd1BEsMjl22
guDNG4ScLWIW4wxNHcOfy8+j5dnPUHw/bhhg1XWnEV9nM5crF5vYfH5X9iWaufAy
SCSKvFpIqm/RGkdnQVU+AMMFItoF4stiF109YJFUaYttYio0Upw=
-----END CERTIFICATE-----"#;

        let key_pem = r#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDBQIhUXd1gXpuL
tHlB8tXDGYQ63sdlvnWrCOxfKBW1yhNUEDQbF0e+Ij/wrHMiyY9P6I2cxty6LxFC
p33gjUeUJhV+2eEkHxsWBP6AnJ2x30i5LmkDQNDr7fVtMAjckHUN16I3f3Q5ntJ2
E55UVR1Tx7bNTKKfd0zRt3AaQF4OKiVROggxriFZxlWlKW4CyXVzv7bLKwuEYSuw
gAFwraM3YfRJZDd+IhJDg7Sku9HBQoVzAh8LdlTdoaZd5AEUrz+PXfw5T+Guqm4T
1ckWBLiHKsShZkNIGiQDIhN/GJpJ2BwymdMi0fc3Krlu2sqtZXOiMsuyjbIDoIDl
EWEEL6ulAgMBAAECggEAHIZTeSx/tC1SxUzGxzK6Vblq+KuQgBacVLoU9bi7d6FT
uAlKP6NwjgKNMI+r0PsyYaegW39I7lxrLkz9ugrwgVAbxSUQ492JiHcFP+OeLTaZ
i+frTTUggWqW2t6HuFLETF5DTfDMrYKhaxdbO/RyRz8H3wbMTEB2QNBURjOxDmLs
9mhxNbg26G4LeCqCoqRkNuqFWxMRfpBq/63BLoOB8K/5tqQkqimNKNf1RQQtjKx+
xe+d37XEs3K8MUfbmCU5atizHMbqjzlFdyZoFM0XzmnNTaXtebWrNfqxeW1odC9Y
HgFJ73QfTwpUgj33iY2XW1qukxuCC7SMyzz3OsBeQwKBgQD9TZ+T89B9L2rCV87j
pcPRbobHdwwB3/9AD4IffBxyUdpcWpMUAxQyn3HEgbmEzr6E6Jf2RhCTn10vNoAa
PcsVDjA/MiUEnePUF6KCkC8rgV/aRNR5B1YGQvg7m9nI4OOOE4D77Cv32DVtUfUD
4QRcnf7sLfyM2Dd6bliexV73FwKBgQDDTz14MZoJFRGSQ7VDtqsVorrurX4KsnoX
+uWKFmxC+shTwiZAixlS8pKTqKdhr5LZj5rD+V5+KhO6aW4RNVHfADP4xW4SxUdA
YUUXyCKf5C4wi6tdQdWEt+Veahv7Mbrr9CleemGzOsPSTAR3q5mh+wgkpcORgfwg
Ld9e5MFoowKBgQDl9lTL02wSWrwHmARB9Dokpr1B1ThXc26eT/YIc3q35svhUHF6
l5j8pHh6uHMeuTuKGkfr04w1GVdWB5qhODxo7yqqFPI6kMVHxfVJp3DLhHbrB9YF
0r0sjhwisck0b8bnM5nEHJOGPQm0J9XTIbP+CYpoDQ/dJmanhgp6iiE/HQKBgGJW
tZadMve7ufsxSEVt5jqgkwq2JC5yqvMECys6GwymhNNXgDcjUn7nUFI0qwKOipws
qDpghulzejdz+k2D0VM9IO3zSnb9CeEqmMVeqcBj/bXHvWLZUQ7gIQcm2iviYEGJ
0IKXkDXUMuDiEaXHqzVZ1kHNjOjoz+/L6Ro4iAGNAoGBAPAa1H7cRk6yH58MXYlz
XjzwwwOewgCko0L1a2UsmD7s2/YnPvkyIXLMY4WXrxX+svQ2/M2ypeRgf0yXo47u
nwJjWbaapHaSBIup3HE/+z8pSlK7tGvtOXmExxLYAF6ET+1Di0rlSoh8ePEhuoA1
Gkd9+8VebP00iWI2zrFdgXE4
-----END PRIVATE KEY-----"#;

        let cert_pem_parsed = pem::parse(cert_pem).expect("Failed to parse cert PEM");
        let key_pem_parsed = pem::parse(key_pem).expect("Failed to parse key PEM");
        
        let svid = X509Svid::parse_from_der(&cert_pem_parsed.contents, &key_pem_parsed.contents).expect("Failed to parse SVID");
        
        write_svid_to_files(&svid, cert_dir, "svid.pem", "svid_key.pem").await.expect("Failed to write files");
        
        let saved_cert = fs::read_to_string(cert_dir.join("svid.pem")).unwrap();
        let saved_key = fs::read_to_string(cert_dir.join("svid_key.pem")).unwrap();
        
        // Ensure they contain the PEM headers
        assert!(saved_cert.contains("CERTIFICATE"));
        assert!(saved_key.contains("PRIVATE KEY"));
    }
}
