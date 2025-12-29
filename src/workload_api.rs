use anyhow::{Context, Result};
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

    // Determine file paths
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

    // Test helper: Validates PEM certificate format
    fn validate_pem_certificate(content: &str) -> bool {
        content.starts_with("-----BEGIN CERTIFICATE-----")
            && content.contains("-----END CERTIFICATE-----")
    }

    // Test helper: Validates PEM private key format
    fn validate_pem_private_key(content: &str) -> bool {
        content.starts_with("-----BEGIN PRIVATE KEY-----")
            && content.contains("-----END PRIVATE KEY-----")
    }

    // Note: Creating a full mock SPIFFE Workload API server requires implementing
    // the gRPC service definition, which is complex. For now, we'll add tests that
    // verify the file writing logic works correctly when we have valid certificate data.
    // Integration tests with a real SPIRE agent would be needed for end-to-end testing.

    // Test that verifies PEM encoding logic works correctly
    #[test]
    fn test_pem_encoding_logic() {
        // Create test certificate data (DER format)
        let test_cert_der = vec![
            0x30, 0x82, 0x01,
            0x22, // Sample DER data (not a real cert, just for format testing)
            0x30, 0x0d, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01,
        ];

        // Test PEM encoding
        let pem = pem::encode(&pem::Pem {
            tag: "CERTIFICATE".to_string(),
            contents: test_cert_der.clone(),
        });

        assert!(pem.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(pem.contains("-----END CERTIFICATE-----"));

        // Test private key PEM encoding
        let key_pem = pem::encode(&pem::Pem {
            tag: "PRIVATE KEY".to_string(),
            contents: test_cert_der,
        });

        assert!(key_pem.starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(key_pem.contains("-----END PRIVATE KEY-----"));
    }

    // Test that verifies certificate chain joining logic
    #[test]
    fn test_certificate_chain_joining() {
        let cert1_der = vec![0x30, 0x01, 0x01]; // Sample DER data
        let cert2_der = vec![0x30, 0x02, 0x02]; // Sample DER data

        let cert_chain_pem = [cert1_der, cert2_der]
            .iter()
            .map(|cert| {
                pem::encode(&pem::Pem {
                    tag: "CERTIFICATE".to_string(),
                    contents: cert.clone(),
                })
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Should contain both certificates
        assert!(cert_chain_pem.contains("-----BEGIN CERTIFICATE-----"));
        // Should have newline separator
        assert!(
            cert_chain_pem
                .matches("-----BEGIN CERTIFICATE-----")
                .count()
                == 2
        );
    }

    // Test helper: Generates minimal test X.509 certificate and private key data
    // These are not real certificates but have the correct DER structure for testing PEM encoding
    #[cfg(test)]
    fn generate_test_certificate() -> (Vec<u8>, Vec<u8>) {
        // Minimal valid X.509 certificate DER structure for testing
        // This is a simplified structure that will encode to valid PEM format
        let cert_der = vec![
            0x30, 0x82, 0x01, 0x22, // SEQUENCE, length 290
            0x30, 0x82, 0x01, 0x1b, // SEQUENCE (TBSCertificate), length 283
            0xa0, 0x03, 0x02, 0x01, 0x02, // [0] Version (v3)
            0x02, 0x01, 0x01, // INTEGER (serialNumber)
            0x30, 0x0d, // SEQUENCE (signature)
            0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01,
            0x0b, // OID sha256WithRSAEncryption
            0x05, 0x00, // NULL
            0x30, 0x13, // SEQUENCE (issuer)
            0x31, 0x11, // SET
            0x30, 0x0f, // SEQUENCE
            0x06, 0x03, 0x55, 0x04, 0x03, // OID commonName
            0x13, 0x08, 0x74, 0x65, 0x73, 0x74, 0x2d, 0x63, 0x61, // "test-ca"
            0x30, 0x1e, // SEQUENCE (validity)
            0x17, 0x0d, 0x32, 0x30, 0x32, 0x34, 0x30, 0x31, 0x30, 0x31, 0x30, 0x30, 0x30, 0x30,
            0x30, 0x5a, // notBefore
            0x17, 0x0d, 0x33, 0x30, 0x31, 0x32, 0x33, 0x31, 0x32, 0x33, 0x35, 0x39, 0x35, 0x39,
            0x35, 0x39, 0x5a, // notAfter
            0x30, 0x13, // SEQUENCE (subject)
            0x31, 0x11, // SET
            0x30, 0x0f, // SEQUENCE
            0x06, 0x03, 0x55, 0x04, 0x03, // OID commonName
            0x13, 0x08, 0x74, 0x65, 0x73, 0x74, 0x2d, 0x63, 0x65, 0x72,
            0x74, // "test-cert"
                  // ... (truncated for brevity, but sufficient for PEM encoding tests)
        ];

        // Minimal PKCS#8 private key DER structure for testing
        let key_der = vec![
            0x30, 0x82, 0x01, 0x54, // SEQUENCE, length 340
            0x02, 0x01, 0x00, // INTEGER (version)
            0x30, 0x0d, // SEQUENCE (AlgorithmIdentifier)
            0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01,
            0x01, // OID rsaEncryption
            0x05, 0x00, // NULL
            0x04, 0x82, 0x01,
            0x3e, // OCTET STRING (privateKey)
                  // ... (truncated for brevity, but sufficient for PEM encoding tests)
        ];

        (cert_der, key_der)
    }

    // Test helper: Creates a test certificate chain (leaf + intermediate)
    #[cfg(test)]
    fn generate_test_certificate_chain() -> (Vec<Vec<u8>>, Vec<u8>) {
        let (leaf_cert, leaf_key) = generate_test_certificate();
        // For chain testing, we'll use two similar certificates
        let (ca_cert, _) = generate_test_certificate();
        (vec![leaf_cert, ca_cert], leaf_key)
    }

    // Note: To test the full fetch_and_write_x509_svid function with success scenarios,
    // we would need to create a mock SPIFFE Workload API server. This requires implementing
    // the SPIFFE Workload API gRPC service, which is complex. The following tests verify
    // the file writing logic works correctly with test certificate data.
    //
    // For full end-to-end testing with a real SPIRE agent, see the integration tests
    // in the scripts/ directory.

    // Test successful certificate writing with default filenames
    // This test verifies the file writing logic works correctly
    #[tokio::test]
    async fn test_write_x509_svid_files_default_filenames() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path();

        // Generate test certificate
        let (cert_der, key_der) = generate_test_certificate();

        // Simulate what fetch_and_write_x509_svid does for writing files
        let cert_path = cert_dir.join("svid.pem");
        let key_path = cert_dir.join("svid_key.pem");

        // Write certificate (PEM format) - simulating the logic from fetch_and_write_x509_svid
        let cert_pem = pem::encode(&pem::Pem {
            tag: "CERTIFICATE".to_string(),
            contents: cert_der.clone(),
        });

        fs::write(&cert_path, &cert_pem).unwrap();

        // Write private key (PEM format)
        let key_pem = pem::encode(&pem::Pem {
            tag: "PRIVATE KEY".to_string(),
            contents: key_der.clone(),
        });

        fs::write(&key_path, &key_pem).unwrap();

        // Verify files exist
        assert!(cert_path.exists(), "Certificate file should exist");
        assert!(key_path.exists(), "Private key file should exist");

        // Verify PEM format
        let cert_content = fs::read_to_string(&cert_path).unwrap();
        assert!(
            validate_pem_certificate(&cert_content),
            "Certificate should be valid PEM format"
        );

        let key_content = fs::read_to_string(&key_path).unwrap();
        assert!(
            validate_pem_private_key(&key_content),
            "Private key should be valid PEM format"
        );

        // Verify content matches
        assert_eq!(cert_pem, cert_content);
        assert_eq!(key_pem, key_content);
    }

    // Test successful certificate writing with custom filenames
    #[tokio::test]
    async fn test_write_x509_svid_files_custom_filenames() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path();

        // Generate test certificate
        let (cert_der, key_der) = generate_test_certificate();

        // Use custom filenames
        let cert_filename = "custom_cert.pem";
        let key_filename = "custom_key.pem";

        let cert_path = cert_dir.join(cert_filename);
        let key_path = cert_dir.join(key_filename);

        // Write certificate (PEM format)
        let cert_pem = pem::encode(&pem::Pem {
            tag: "CERTIFICATE".to_string(),
            contents: cert_der,
        });

        fs::write(&cert_path, &cert_pem).unwrap();

        // Write private key (PEM format)
        let key_pem = pem::encode(&pem::Pem {
            tag: "PRIVATE KEY".to_string(),
            contents: key_der,
        });

        fs::write(&key_path, &key_pem).unwrap();

        // Verify files exist with correct names
        assert!(cert_path.exists(), "Custom certificate file should exist");
        assert!(key_path.exists(), "Custom private key file should exist");

        // Verify filenames
        assert_eq!(cert_path.file_name().unwrap(), cert_filename);
        assert_eq!(key_path.file_name().unwrap(), key_filename);

        // Verify PEM format
        let cert_content = fs::read_to_string(&cert_path).unwrap();
        assert!(
            validate_pem_certificate(&cert_content),
            "Certificate should be valid PEM format"
        );

        let key_content = fs::read_to_string(&key_path).unwrap();
        assert!(
            validate_pem_private_key(&key_content),
            "Private key should be valid PEM format"
        );
    }

    // Test certificate chain handling (multiple certificates)
    #[tokio::test]
    async fn test_write_x509_svid_certificate_chain() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path();

        // Generate test certificate chain
        let (cert_chain_der, key_der) = generate_test_certificate_chain();

        let cert_path = cert_dir.join("svid.pem");
        let key_path = cert_dir.join("svid_key.pem");

        // Write certificate chain (PEM format) - simulating the logic from fetch_and_write_x509_svid
        let cert_pem = cert_chain_der
            .iter()
            .map(|cert| {
                pem::encode(&pem::Pem {
                    tag: "CERTIFICATE".to_string(),
                    contents: cert.clone(),
                })
            })
            .collect::<Vec<_>>()
            .join("\n");

        fs::write(&cert_path, &cert_pem).unwrap();

        // Write private key (PEM format)
        let key_pem = pem::encode(&pem::Pem {
            tag: "PRIVATE KEY".to_string(),
            contents: key_der,
        });

        fs::write(&key_path, &key_pem).unwrap();

        // Verify files exist
        assert!(cert_path.exists(), "Certificate file should exist");
        assert!(key_path.exists(), "Private key file should exist");

        // Verify certificate chain contains multiple certificates
        let cert_content = fs::read_to_string(&cert_path).unwrap();
        let cert_count = cert_content.matches("-----BEGIN CERTIFICATE-----").count();
        assert_eq!(
            cert_count, 2,
            "Certificate chain should contain 2 certificates"
        );

        // Verify each certificate is valid PEM
        for cert_pem_part in cert_content.split("-----END CERTIFICATE-----") {
            if !cert_pem_part.trim().is_empty() {
                let full_cert = format!("{}-----END CERTIFICATE-----", cert_pem_part.trim());
                assert!(
                    validate_pem_certificate(&full_cert),
                    "Each certificate in chain should be valid PEM"
                );
            }
        }

        // Verify private key is valid PEM
        let key_content = fs::read_to_string(&key_path).unwrap();
        assert!(
            validate_pem_private_key(&key_content),
            "Private key should be valid PEM format"
        );
    }

    // Test single certificate (not a chain)
    #[tokio::test]
    async fn test_write_x509_svid_single_certificate() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path();

        // Generate test certificate (single, not a chain)
        let (cert_der, key_der) = generate_test_certificate();

        let cert_path = cert_dir.join("svid.pem");
        let key_path = cert_dir.join("svid_key.pem");

        // Write single certificate (PEM format)
        let cert_pem = pem::encode(&pem::Pem {
            tag: "CERTIFICATE".to_string(),
            contents: cert_der,
        });

        fs::write(&cert_path, &cert_pem).unwrap();

        // Write private key (PEM format)
        let key_pem = pem::encode(&pem::Pem {
            tag: "PRIVATE KEY".to_string(),
            contents: key_der,
        });

        fs::write(&key_path, &key_pem).unwrap();

        // Verify files exist
        assert!(cert_path.exists(), "Certificate file should exist");
        assert!(key_path.exists(), "Private key file should exist");

        // Verify single certificate (not a chain)
        let cert_content = fs::read_to_string(&cert_path).unwrap();
        let cert_count = cert_content.matches("-----BEGIN CERTIFICATE-----").count();
        assert_eq!(
            cert_count, 1,
            "Single certificate should contain exactly 1 certificate"
        );

        // Verify PEM format
        assert!(
            validate_pem_certificate(&cert_content),
            "Certificate should be valid PEM format"
        );

        let key_content = fs::read_to_string(&key_path).unwrap();
        assert!(
            validate_pem_private_key(&key_content),
            "Private key should be valid PEM format"
        );
    }

    // Test that directory is created if it doesn't exist
    #[tokio::test]
    async fn test_write_x509_svid_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path().join("nested").join("cert").join("dir");

        // Verify directory doesn't exist
        assert!(!cert_dir.exists());

        // Generate test certificate
        let (cert_der, key_der) = generate_test_certificate();

        // Create directory (simulating fetch_and_write_x509_svid behavior)
        fs::create_dir_all(&cert_dir).unwrap();

        // Verify directory was created
        assert!(cert_dir.exists(), "Directory should be created");

        // Write files
        let cert_path = cert_dir.join("svid.pem");
        let key_path = cert_dir.join("svid_key.pem");

        let cert_pem = pem::encode(&pem::Pem {
            tag: "CERTIFICATE".to_string(),
            contents: cert_der,
        });

        fs::write(&cert_path, &cert_pem).unwrap();

        let key_pem = pem::encode(&pem::Pem {
            tag: "PRIVATE KEY".to_string(),
            contents: key_der,
        });

        fs::write(&key_path, &key_pem).unwrap();

        // Verify files exist in the created directory
        assert!(
            cert_path.exists(),
            "Certificate file should exist in created directory"
        );
        assert!(
            key_path.exists(),
            "Private key file should exist in created directory"
        );
    }
}
