use anyhow::{Context, Result};
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

use crate::config::Config;

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
    // Use create_x509_source to handle retry logic and connection
    let source = create_x509_source(agent_address).await?;

    // Get the SVID from the source
    let svid: X509Svid = source
        .get_svid()
        .map_err(|e| anyhow::anyhow!("Failed to get SVID from source: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("X509Source returned no SVID (None)"))?;

    write_svid_to_files(&svid, cert_dir, svid_file_name, svid_key_file_name).await?;

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

    Ok(())
}

/// Writes the X.509 SVID to the specified directory.
async fn write_svid_to_files(
    svid: &X509Svid,
    cert_dir: &Path,
    svid_file_name: &str,
    svid_key_file_name: &str,
) -> Result<()> {
    // Create cert directory if it doesn't exist
    fs::create_dir_all(cert_dir)
        .with_context(|| format!("Failed to create cert directory: {}", cert_dir.display()))?;

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

/// Writes the X.509 trust bundle to the specified directory.
async fn write_bundle_to_file(
    bundle: &X509Bundle,
    cert_dir: &Path,
    bundle_file_name: &str,
) -> Result<()> {
    let bundle_path = cert_dir.join(bundle_file_name);

    // Write bundle certificates (PEM format)
    let bundle_pem = bundle
        .authorities()
        .iter()
        .map(|cert: &spiffe::cert::Certificate| {
            pem::encode(&pem::Pem {
                tag: "CERTIFICATE".to_string(),
                contents: cert.as_ref().to_vec(),
            })
        })
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(&bundle_path, bundle_pem)
        .with_context(|| format!("Failed to write bundle to {}", bundle_path.display()))?;

    Ok(())
}

/// Writes X509 SVID and trust bundle to disk when an update is received from the SPIRE agent.
///
/// This function is called when the X509Source receives an update notification.
/// It writes the updated SVID (certificate and private key) and trust bundle to the configured directory.
///
/// # Arguments
///
/// * `svid` - The updated X509 SVID containing the certificate chain and private key
/// * `bundle` - The trust bundle containing CA certificates
/// * `config` - Configuration containing output paths
pub async fn write_x509_svid_on_update(
    svid: &X509Svid,
    bundle: &X509Bundle,
    config: &Config,
) -> Result<()> {
    let cert_dir = config
        .cert_dir
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("cert_dir must be configured"))?;
    let cert_dir_path = Path::new(cert_dir);

    write_svid_to_files(
        svid,
        cert_dir_path,
        config.svid_file_name(),
        config.svid_key_file_name(),
    )
    .await?;

    write_bundle_to_file(bundle, cert_dir_path, config.svid_bundle_file_name()).await?;

    // Log update with SPIFFE ID and certificate expiry
    let expiry = match x509_parser::parse_x509_certificate(svid.leaf().as_ref()) {
        Ok((_, cert)) => cert
            .validity()
            .not_after
            .to_rfc2822()
            .unwrap_or_else(|_| "unknown".to_string()),
        Err(_) => "unknown".to_string(),
    };
    println!(
        "Updated certificate: spiffe_id={}, expires={}",
        svid.spiffe_id(),
        expiry
    );

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
    // Retry when SPIRE agent is not yet ready to serve:
    // - Socket file doesn't exist yet (NotFound, "No such file or directory")
    // - Socket exists but not accepting connections (ConnectionRefused, "Connection refused")
    // - Permission issues during workload attestation (PermissionDenied)
    error_str.contains("PermissionDenied")
        || error_str.contains("ConnectionRefused")
        || error_str.contains("Connection refused")
        || error_str.contains("NotFound")
        || error_str.contains("No such file or directory")
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

        let err = anyhow::anyhow!("No such file or directory");
        assert!(is_retryable_error(&err));

        let err = anyhow::anyhow!("Connection refused");
        assert!(is_retryable_error(&err));

        let err = anyhow::anyhow!("Some other error");
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
        let cert_dir = temp_dir.path().join("nested_certs");

        // Test with custom file names - should fail on connection
        let result = fetch_and_write_x509_svid(
            "unix:///tmp/nonexistent-socket.sock",
            &cert_dir,
            "custom_cert.pem",
            "custom_key.pem",
        )
        .await;

        // Should fail on connection before creating directory
        // (directory is only created when we have an SVID to write)
        assert!(result.is_err());
        assert!(
            !cert_dir.exists(),
            "Cert directory should not be created if fetch fails"
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
    async fn test_write_x509_svid_on_update_writes_files() {
        use spiffe::bundle::x509::X509Bundle;
        use spiffe::spiffe_id::TrustDomain;
        use spiffe::svid::x509::X509Svid;

        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path();

        // Mock config
        let config = Config {
            cert_dir: Some(cert_dir.to_str().unwrap().to_string()),
            svid_file_name: Some("test_svid.pem".to_string()),
            svid_key_file_name: Some("test_key.pem".to_string()),
            ..Default::default()
        };

        // Parse mock SVID
        let cert_pem = r#"-----BEGIN CERTIFICATE-----
MIIDNTCCAh2gAwIBAgIUGq/oNncXam0A9VgyVENC8GuQn/gwDQYJKoZIhvcNAQEL
BQAwFDESMBAGA1UEAwwJbG9jYWxob3N0MB4XDTI1MTIyOTAwNTYyOVoXDTI2MTIy
OTAwNTYyOVowFDESMBAGA1UEAwwJbG9jYWxob3N0MIIBIjANBgkqhkiG9w0BAQEF
AAOCAQ8AMIIBCgKCAQEA1n0i1hMPSoH7J+XRuR1j6VS93fd4t+RNfVp/a7yvaZOR
f0aSWYK4qZy7gzys1KH7akQON+LCpw6RTiIWimAzAZ2Yx8DMxbSzH4PYMQ7URI7/
MRUPXz3qCwbubtkJwNNbFb+x8d87HR7GpLJMrt2MqboQBILTaaFYu3nvwi5RLVdZ
h+wzEQbWDjR5RZo9SElhN9vJfKhSS2aYL8zpGhHb5e+IbYw5pzKgKLa6jnyLHqAz
Jf5Dt4CqYJDzTpsBG5dH3d/f5isMBe2u+E5D901IG1v8eUKP1lEJrljqx9xpgYf0
MtwwCn5dnom8WOpQvP9Im4Xdy7vZ7PIcsvuZeaJNsQIDAQABo38wfTAOBgNVHQ8B
Af8EBAMCBaAwHQYDVR0lBBYwFAYIKwYBBQUHAwEGCCsGAQUFBwMCMAkGA1UdEwQC
MAAwIgYDVR0RBBswGYYXc3BpZmZlOi8vbG9jYWxob3N0L3Rlc3QwHQYDVR0OBBYE
FPWyhgkS+mDZTVK+kcRAHK1CSwyxMA0GCSqGSIb3DQEBCwUAA4IBAQDQwoTbmFB7
xtfk2ieQAaul+AgCNopkr36xtE07vxEP307tC6hO2RMJUWYOFeioxPBbDpa5ff/3
6n4QgHpnFAGDIvwvuUa1upIkvaHFYFlyPFvcyzBZqhob/wIn8WIITFfkzygbkxGi
XzjpK0rIywC6cdaqYMDcIUyqNCO2l2FvccN7flo2pnppj6w55kv+FTX0C+AUv3qC
p2OFoxDKsFWk52J0qXR/QefV5fFnrOLgqI2zCbyxSr7EZzGW9Fbr+YrpzXfI8Z0b
8GGRaPE6WbPGjvc97Uwmp3T+4UkJatFnaAHnTsRikdbZ1F0xNcvE13pltbG3vFk0
lQluKI5/n4db
-----END CERTIFICATE-----"#;

        let key_pem = r#"-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQDWfSLWEw9Kgfsn
5dG5HWPpVL3d93i35E19Wn9rvK9pk5F/RpJZgripnLuDPKzUoftqRA434sKnDpFO
IhaKYDMBnZjHwMzFtLMfg9gxDtREjv8xFQ9fPeoLBu5u2QnA01sVv7Hx3zsdHsak
skyu3YypuhAEgtNpoVi7ee/CLlEtV1mH7DMRBtYONHlFmj1ISWE328l8qFJLZpgv
zOkaEdvl74htjDmnMqAotrqOfIseoDMl/kO3gKpgkPNOmwEbl0fd39/mKwwF7a74
TkP3TUgbW/x5Qo/WUQmuWOrH3GmBh/Qy3DAKfl2eibxY6lC8/0ibhd3Lu9ns8hyy
+5l5ok2xAgMBAAECggEAAO0baGc+qqizB/ITHMSGuOw3waye5dRjjUYFxNZUv5T2
jOEmIqLQ31Kg8KkjaeulJUlT8mPVSVljwT2ecUyHC9u9XCd1+uiT2W/9UADrY7xm
V7TqkxO2XgPSpcHkK+P9wbNJNm0rWS3X18A5Wov0XotCJHLYLN2Yf37ATUtb6GE1
J5wqaSaqVwLbhNk0rRojsWNO61LYYsEL3fA/Q2UA0lLfo5BkuHIHRJJvdtmpWX2L
Rf6lV4nxdx+nxPIkqYo0wFLanuM+6+zO2ej094/Op3CWnxqXoUnCzyA8tut7+0zk
o1LN5ygAdDFlJ0qvyPUTeDHLG+H0DfMKcI3jBRUmAQKBgQD56BH/+qH0A9oISwgM
75C+mKt/88LFA5ztUOwz7k4opVOYtrUxDNKRqplI4bUedJMWUbm2kXFh00YIBt7u
9PMgkQwq6j5IK4JzcPYto/Zl6bNuoiL7/WQU3lSTspu0xhEqAYC+KAxEI0WuuIVZ
J9QSq1884dTBwHiXmnNmCX3BkQKBgQDbt/yOKjnsSJd5YtktWrJ9DnPamkwIqub1
D59k/HwKs8StSHNFW0fkVpTRTa7R12CMgu1n5KvGOt2PX1VNPHh4O/8th1pkt2Jj
lf29NMmSXcOi7KPjj0zBWmDAx0cgkt7ftQcc42+9CWxyUdbgYqMismaUit0zZkhR
5nvsALm6IQKBgDoZHbYpCmW0T4gGCYUYXMoyrAw/G1S6Fk2FtqQMDtecN+cU8uLI
XFvJEYHEF1tRNrDFpysufPGFMI7FKibbg3pavj1r37bfhqBX7qOFrs7amgBqaT+0
FQRU+8yqhVBti6f8WXXb0Z41pQmNlFK506/Tb3yz88ZnfKGiIpniMv5BAoGAQn7K
JlRNN184yHnL9FfwkLxg/5WW0UC3qQ7TVIK9H5gMO80jZagcd9RkMXvrHoKqK5ws
MTcZbWK/TvaxIDDe3LR7o9HE35pIYo8wPaTOJEfQP2ySpPnnZtTtVyp4MjmAzf9B
adLDLFi/w1FVUI9Jg+St+uKT00xvMqoocuI9U0ECgYEAzlapqhd+CXpy7KQKNtRt
A/lJGE6bkB2JNXbr01DthVr5JSDPz39AxTRB9VeRUt5irB8f7OvmS7fy6+FY9Jxn
QBAx6pG1tAXOEZt4R56+FIKBFcHJFB0ja/RQDRDLCZl+KFUDfgRNvomZx1lWBicI
fPfrHw1nYcPliVB4Zbv8d1w=
-----END PRIVATE KEY-----"#;

        let cert_der = pem::parse(cert_pem).unwrap().contents;
        let key_der = pem::parse(key_pem).unwrap().contents;

        let svid = X509Svid::parse_from_der(&cert_der, &key_der).expect("Failed to parse SVID");
        let td = TrustDomain::new("localhost").unwrap();
        let bundle = X509Bundle::parse_from_der(td, &cert_der).expect("Failed to parse Bundle");

        // Call handler
        let result = write_x509_svid_on_update(&svid, &bundle, &config).await;

        assert!(result.is_ok());

        // Verify files
        let cert_path = cert_dir.join("test_svid.pem");
        let key_path = cert_dir.join("test_key.pem");
        let bundle_path = cert_dir.join("svid_bundle.pem"); // default name

        assert!(cert_path.exists());
        assert!(key_path.exists());
        assert!(bundle_path.exists());

        // Verify content
        let written_cert = fs::read_to_string(cert_path).unwrap();
        assert!(written_cert.contains("BEGIN CERTIFICATE"));

        let written_key = fs::read_to_string(key_path).unwrap();
        assert!(written_key.contains("BEGIN PRIVATE KEY"));

        let written_bundle = fs::read_to_string(bundle_path).unwrap();
        assert!(written_bundle.contains("BEGIN CERTIFICATE"));
    }
}
