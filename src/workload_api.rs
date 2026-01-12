use anyhow::{Context, Result};
use spiffe::bundle::x509::X509Bundle;
use spiffe::svid::x509::X509Svid;
use spiffe::svid::SvidSource;
use spiffe::workload_api::client::WorkloadApiClient;
use spiffe::workload_api::x509_source::{X509Source, X509SourceBuilder};
use std::fs;
use std::iter::Take;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio_retry::strategy::ExponentialBackoff;
use tokio_retry::RetryIf;

use crate::cli::Config;

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
    let factory = X509SourceFactory::new().with_address(agent_address);
    fetch_and_write_x509_svid_with_factory(&factory, cert_dir, svid_file_name, svid_key_file_name)
        .await
}

async fn fetch_and_write_x509_svid_with_factory(
    factory: &X509SourceFactory,
    cert_dir: &Path,
    svid_file_name: &str,
    svid_key_file_name: &str,
) -> Result<()> {
    let source = factory.create().await?;

    // Get the SVID from the source
    let svid: X509Svid = source
        .get_svid()
        .map_err(|e| anyhow::anyhow!("Failed to get SVID from source: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("X509Source returned no SVID (None)"))?;

    write_svid_to_files(&svid, cert_dir, svid_file_name, svid_key_file_name)?;

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
fn write_svid_to_files(
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
fn write_bundle_to_file(
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
/// This function is called when the `X509Source` receives an update notification.
/// It writes the updated SVID (certificate and private key) and trust bundle to the configured directory.
///
/// # Arguments
///
/// * `svid` - The updated X509 SVID containing the certificate chain and private key
/// * `bundle` - The trust bundle containing CA certificates
/// * `config` - Configuration containing output paths
pub fn write_x509_svid_on_update(
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
    )?;

    write_bundle_to_file(bundle, cert_dir_path, config.svid_bundle_file_name())?;

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

pub struct X509SourceFactory {
    retry_strategy: Take<ExponentialBackoff>,
    address: String,
}

impl Default for X509SourceFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl X509SourceFactory {
    pub fn new() -> Self {
        let retry_strategy = ExponentialBackoff::from_millis(1000)
            .max_delay(Duration::from_secs(16))
            .take(10);

        Self {
            retry_strategy,
            address: String::new(),
        }
    }

    pub async fn create(&self) -> Result<Arc<X509Source>> {
        if self.address.is_empty() {
            return Err(anyhow::anyhow!("address must be configured"));
        }

        let address = self.address.clone();
        let mut attempt = 0u32;

        RetryIf::spawn(
            self.retry_strategy.clone(),
            || {
                let addr = address.clone();
                attempt += 1;
                async move {
                    if attempt > 1 {
                        eprintln!(
                            "Retry attempt {attempt} to create X509Source (address={addr})"
                        );
                    }
                    let client = create_workload_api_client(&addr).await?;
                    X509SourceBuilder::new()
                        .with_client(client)
                        .build()
                        .await
                        .context("Failed to build X509Source")
                }
            },
            |err: &anyhow::Error| {
                let retryable = is_retryable_error(err);
                if !retryable {
                    let error_category = categorize_error(err);
                    eprintln!(
                        "Non-retryable error during X509Source creation: category={error_category}, error={err:?}"
                    );
                }
                retryable
            },
        )
        .await
        .map_err(|e| {
            let error_category = categorize_error(&e);
            anyhow::anyhow!(
                "Failed to create X509Source from SPIRE agent after {attempt} attempts: category={error_category}, error={e}"
            )
        })
    }

    pub fn with_retry(mut self, strategy: Take<ExponentialBackoff>) -> Self {
        self.retry_strategy = strategy;
        self
    }

    pub fn with_address(mut self, address: &str) -> Self {
        self.address = address.to_string();
        self
    }
}

async fn create_workload_api_client(address: &str) -> Result<WorkloadApiClient> {
    let address = address
        .strip_prefix(UDS_PREFIX)
        .map_or_else(|| String::from(address), |v| format!("unix:{v}"));

    WorkloadApiClient::new_from_path(&address)
        .await
        .with_context(|| format!("Failed to create WorkloadApiClient from address: {address}"))
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

/// Categorizes an error into a human-readable category for diagnostic purposes.
/// This helps operators quickly identify whether an error is related to configuration,
/// network connectivity, or other issues.
fn categorize_error(err: &anyhow::Error) -> &'static str {
    let error_str = format!("{err:?}");

    // Network/connectivity errors (retryable)
    if error_str.contains("ConnectionRefused") || error_str.contains("Connection refused") {
        return "network/connection_refused";
    }
    if error_str.contains("NotFound") || error_str.contains("No such file or directory") {
        return "network/socket_not_found";
    }
    if error_str.contains("PermissionDenied") {
        return "network/permission_denied";
    }

    // gRPC/protocol errors
    if error_str.contains("tonic") || error_str.contains("grpc") || error_str.contains("h2") {
        return "grpc/protocol_error";
    }

    // Configuration errors
    if error_str.contains("InvalidUri")
        || error_str.contains("invalid")
        || error_str.contains("Invalid")
    {
        return "config/invalid_address";
    }

    // SPIFFE-specific errors
    if error_str.contains("SVID") || error_str.contains("spiffe") {
        return "spiffe/svid_error";
    }

    "unknown"
}

#[cfg(test)]
mod tests {
    use super::*;
    use spiffe::bundle::x509::X509Bundle;
    use spiffe::spiffe_id::TrustDomain;
    use spiffe::svid::x509::X509Svid;
    use std::fs;
    use std::time::Instant;
    use tempfile::TempDir;

    const TEST_CERT_PEM: &str = r"-----BEGIN CERTIFICATE-----
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
-----END CERTIFICATE-----";

    const TEST_KEY_PEM: &str = r"-----BEGIN PRIVATE KEY-----
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
-----END PRIVATE KEY-----";

    fn get_test_svid() -> X509Svid {
        let cert_der = pem::parse(TEST_CERT_PEM).unwrap().contents;
        let key_der = pem::parse(TEST_KEY_PEM).unwrap().contents;
        X509Svid::parse_from_der(&cert_der, &key_der).expect("Failed to parse SVID")
    }

    fn get_test_bundle() -> X509Bundle {
        let cert_der = pem::parse(TEST_CERT_PEM).unwrap().contents;
        let td = TrustDomain::new("localhost").unwrap();
        X509Bundle::parse_from_der(td, &cert_der).expect("Failed to parse Bundle")
    }

    #[test]
    fn test_is_retryable_error() {
        let err = anyhow::anyhow!("Some PermissionDenied error");
        assert!(is_retryable_error(&err));

        let err = anyhow::anyhow!("NotFound: file not found");
        assert!(is_retryable_error(&err));

        let err = anyhow::anyhow!("ConnectionRefused: refused");
        assert!(is_retryable_error(&err));

        let err = anyhow::anyhow!("No such file or directory");
        assert!(is_retryable_error(&err));

        let err = anyhow::anyhow!("Connection refused");
        assert!(is_retryable_error(&err));

        let err = anyhow::anyhow!("Some other error");
        assert!(!is_retryable_error(&err));
    }

    #[test]
    fn test_categorize_error_network_errors() {
        let err = anyhow::anyhow!("ConnectionRefused: refused");
        assert_eq!(categorize_error(&err), "network/connection_refused");

        let err = anyhow::anyhow!("Connection refused by server");
        assert_eq!(categorize_error(&err), "network/connection_refused");

        let err = anyhow::anyhow!("NotFound: socket not found");
        assert_eq!(categorize_error(&err), "network/socket_not_found");

        let err = anyhow::anyhow!("No such file or directory");
        assert_eq!(categorize_error(&err), "network/socket_not_found");

        let err = anyhow::anyhow!("PermissionDenied: access denied");
        assert_eq!(categorize_error(&err), "network/permission_denied");
    }

    #[test]
    fn test_categorize_error_grpc_errors() {
        let err = anyhow::anyhow!("tonic error: transport error");
        assert_eq!(categorize_error(&err), "grpc/protocol_error");

        let err = anyhow::anyhow!("grpc status code: unavailable");
        assert_eq!(categorize_error(&err), "grpc/protocol_error");

        let err = anyhow::anyhow!("h2 protocol error");
        assert_eq!(categorize_error(&err), "grpc/protocol_error");
    }

    #[test]
    fn test_categorize_error_config_errors() {
        let err = anyhow::anyhow!("InvalidUri: bad uri format");
        assert_eq!(categorize_error(&err), "config/invalid_address");

        let err = anyhow::anyhow!("invalid address format");
        assert_eq!(categorize_error(&err), "config/invalid_address");

        let err = anyhow::anyhow!("Invalid socket path");
        assert_eq!(categorize_error(&err), "config/invalid_address");
    }

    #[test]
    fn test_categorize_error_spiffe_errors() {
        let err = anyhow::anyhow!("SVID parsing failed");
        assert_eq!(categorize_error(&err), "spiffe/svid_error");

        let err = anyhow::anyhow!("spiffe error occurred");
        assert_eq!(categorize_error(&err), "spiffe/svid_error");
    }

    #[test]
    fn test_categorize_error_unknown() {
        let err = anyhow::anyhow!("Some completely unexpected error");
        assert_eq!(categorize_error(&err), "unknown");
    }

    #[tokio::test]
    async fn test_fetch_and_write_x509_svid_fail_fast() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path();

        let start = Instant::now();
        let factory = test_x509_source_factory("invalid://address");
        let result =
            fetch_and_write_x509_svid_with_factory(&factory, cert_dir, "svid.pem", "svid_key.pem")
                .await;
        let duration = start.elapsed();

        assert!(result.is_err());
        assert!(
            duration < Duration::from_millis(500),
            "Should fail fast on non-retryable error, took {duration:?}"
        );
    }

    #[tokio::test]
    async fn test_fetch_and_write_x509_svid_custom_file_names() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path().join("nested_certs");

        let factory = test_x509_source_factory("unix:///tmp/nonexistent-socket.sock");
        let result = fetch_and_write_x509_svid_with_factory(
            &factory,
            &cert_dir,
            "custom_cert.pem",
            "custom_key.pem",
        )
        .await;

        assert!(result.is_err());
        assert!(!cert_dir.exists());
    }

    fn test_x509_source_factory(address: &str) -> X509SourceFactory {
        X509SourceFactory::new()
            .with_address(address)
            .with_retry(ExponentialBackoff::from_millis(10).take(3))
    }

    #[tokio::test]
    async fn test_x509_source_factory_empty_address() {
        let factory = X509SourceFactory::new();
        let result = factory.create().await;

        match result {
            Ok(_) => panic!("Expected error for empty address"),
            Err(e) => {
                let error_msg = e.to_string();
                assert!(error_msg.contains("address must be configured"));
            }
        }
    }

    #[tokio::test]
    async fn test_x509_source_factory_whitespace_address() {
        let factory = X509SourceFactory::new().with_address("   ");
        let result = factory.create().await;

        match result {
            Ok(_) => panic!("Expected error for whitespace address"),
            Err(e) => {
                let error_msg = e.to_string();
                assert!(!error_msg.contains("address must be configured"));
            }
        }
    }

    #[test]
    fn test_x509_source_factory_builder_methods() {
        let strategy = ExponentialBackoff::from_millis(100).take(1);
        let factory = X509SourceFactory::new()
            .with_address("unix:///tmp/test.sock")
            .with_retry(strategy);

        assert_eq!(factory.address, "unix:///tmp/test.sock");
    }

    #[tokio::test]
    async fn test_create_workload_api_client_unix_prefix_conversion() {
        let result = create_workload_api_client("unix:///tmp/test-socket.sock").await;
        match result {
            Ok(_) => panic!("Expected error"),
            Err(e) => assert!(e.to_string().contains("Failed to create WorkloadApiClient")),
        }
    }

    #[tokio::test]
    async fn test_create_workload_api_client_without_prefix() {
        let result = create_workload_api_client("unix:/tmp/test-socket.sock").await;
        match result {
            Ok(_) => panic!("Expected error"),
            Err(e) => assert!(e.to_string().contains("Failed to create WorkloadApiClient")),
        }
    }

    #[tokio::test]
    async fn test_create_workload_api_client_invalid_address() {
        let result = create_workload_api_client("invalid://address").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_write_svid_to_files_success() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path();
        let svid = get_test_svid();

        write_svid_to_files(&svid, cert_dir, "svid.pem", "svid_key.pem").unwrap();

        assert!(cert_dir.join("svid.pem").exists());
        assert!(cert_dir.join("svid_key.pem").exists());

        let cert_content = fs::read_to_string(cert_dir.join("svid.pem")).unwrap();
        assert!(cert_content.contains("BEGIN CERTIFICATE"));
        let key_content = fs::read_to_string(cert_dir.join("svid_key.pem")).unwrap();
        assert!(key_content.contains("BEGIN PRIVATE KEY"));
    }

    #[tokio::test]
    async fn test_write_bundle_to_file_success() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path();
        let bundle = get_test_bundle();

        write_bundle_to_file(&bundle, cert_dir, "bundle.pem").unwrap();

        assert!(cert_dir.join("bundle.pem").exists());
        let content = fs::read_to_string(cert_dir.join("bundle.pem")).unwrap();
        assert!(content.contains("BEGIN CERTIFICATE"));
    }

    #[tokio::test]
    async fn test_write_x509_svid_on_update_writes_files() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path();

        let config = Config {
            cert_dir: Some(cert_dir.to_str().unwrap().to_string()),
            svid_file_name: Some("test_svid.pem".to_string()),
            svid_key_file_name: Some("test_key.pem".to_string()),
            ..Default::default()
        };

        let svid = get_test_svid();
        let bundle = get_test_bundle();

        let result = write_x509_svid_on_update(&svid, &bundle, &config);
        assert!(result.is_ok());

        assert!(cert_dir.join("test_svid.pem").exists());
        assert!(cert_dir.join("test_key.pem").exists());
        assert!(cert_dir.join("svid_bundle.pem").exists());
    }

    #[tokio::test]
    async fn test_write_x509_svid_on_update_no_cert_dir() {
        let svid = get_test_svid();
        let bundle = get_test_bundle();
        let config = Config::default();

        let result = write_x509_svid_on_update(&svid, &bundle, &config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cert_dir must be configured"));
    }

    #[test]
    fn test_pem_encoding_logic() {
        let data = vec![0x30, 0x01, 0x01];
        let pem = pem::encode(&pem::Pem {
            tag: "CERTIFICATE".to_string(),
            contents: data,
        });
        assert!(pem.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(pem.contains("-----END CERTIFICATE-----"));
    }

    #[test]
    fn test_certificate_chain_joining() {
        let cert1 = vec![0x30, 0x01, 0x01];
        let cert2 = vec![0x30, 0x02, 0x02];

        let chain = [cert1, cert2]
            .iter()
            .map(|c| {
                pem::encode(&pem::Pem {
                    tag: "CERTIFICATE".to_string(),
                    contents: c.clone(),
                })
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(chain.matches("-----BEGIN CERTIFICATE-----").count(), 2);
    }
}
