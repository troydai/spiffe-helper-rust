use anyhow::{Context, Result};
use spiffe::bundle::x509::X509Bundle;
use spiffe::svid::x509::X509Svid;
use spiffe::{X509Source, X509SourceBuilder};
use std::fs;
use std::iter::Take;
use std::path::Path;
use std::sync::Arc;
use tokio_retry::strategy::ExponentialBackoff;
use tokio_retry::RetryIf;

use crate::cli::Config;

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
    let svid = source.svid().map_err(|e| anyhow::anyhow!("SVID error: {}", e))?;
    write_svid_to_files(&svid, cert_dir, svid_file_name, svid_key_file_name)?;
    Ok(())
}

fn write_svid_to_files(
    svid: &X509Svid,
    cert_dir: &Path,
    svid_file_name: &str,
    svid_key_file_name: &str,
) -> Result<()> {
    fs::create_dir_all(cert_dir)?;
    let cert_path = cert_dir.join(svid_file_name);
    let key_path = cert_dir.join(svid_key_file_name);

    let cert_pem = svid
        .cert_chain()
        .iter()
        .map(|cert| pem::encode(&pem::Pem::new("CERTIFICATE", cert.as_ref().to_vec())))
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(&cert_path, cert_pem)?;
    let key_pem = pem::encode(&pem::Pem::new("PRIVATE KEY", svid.private_key().as_ref().to_vec()));
    fs::write(&key_path, key_pem)?;
    Ok(())
}

fn write_bundle_to_file(
    bundle: &X509Bundle,
    cert_dir: &Path,
    bundle_file_name: &str,
) -> Result<()> {
    let bundle_path = cert_dir.join(bundle_file_name);
    let bundle_pem = bundle
        .authorities()
        .iter()
        .map(|cert| pem::encode(&pem::Pem::new("CERTIFICATE", cert.as_ref().to_vec())))
        .collect::<Vec<_>>()
        .join("\n");

    fs::write(&bundle_path, bundle_pem)?;
    Ok(())
}

pub fn write_x509_svid_on_update(
    svid: &X509Svid,
    bundle: &X509Bundle,
    config: &Config,
) -> Result<()> {
    let cert_dir = config.cert_dir.as_ref().ok_or_else(|| anyhow::anyhow!("cert_dir missing"))?;
    let cert_dir_path = Path::new(cert_dir);
    write_svid_to_files(svid, cert_dir_path, config.svid_file_name(), config.svid_key_file_name())?;
    write_bundle_to_file(bundle, cert_dir_path, config.svid_bundle_file_name())?;
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
        let retry_strategy = ExponentialBackoff::from_millis(1000).take(10);
        Self { retry_strategy, address: String::new() }
    }

    pub async fn create(&self) -> Result<Arc<X509Source>> {
        if self.address.is_empty() {
            return Err(anyhow::anyhow!("address empty"));
        }
        let address = self.address.clone();
        RetryIf::spawn(
            self.retry_strategy.clone(),
            || {
                let addr = address.clone();
                async move {
                    X509SourceBuilder::new()
                        .endpoint(&addr)
                        .build()
                        .await
                        .context("X509Source build failed")
                }
            },
            |err: &anyhow::Error| {
                let s = format!("{:?}", err);
                s.contains("ConnectionRefused") || s.contains("NotFound")
            },
        )
        .await
        .map(Arc::new)
        .map_err(|e| anyhow::anyhow!("Retry failed: {}", e))
    }

    pub fn with_address(mut self, address: &str) -> Self {
        self.address = address.to_string();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spiffe::bundle::x509::X509Bundle;
    use spiffe::spiffe_id::TrustDomain;
    use spiffe::svid::x509::X509Svid;
    use std::fs;
    use tempfile::TempDir;

    const TEST_CERT_PEM: &str = "-----BEGIN CERTIFICATE-----\n\
MIIDNTCCAh2gAwIBAgIUGq/oNncXam0A9VgyVENC8GuQn/gwDQYJKoZIhvcNAQEL\n\
BQAwFDESMBAGA1UEAwwJbG9jYWxob3N0MB4XDTI1MTIyOTAwNTYyOVoXDTI2MTIy\n\
OTAwNTYyOVowFDESMBAGA1UEAwwJbG9jYWxob3N0MIIBIjANBgkqhkiG9w0BAQEF\n\
AAOCAQ8AMIIBCgKCAQEA1n0i1hMPSoH7J+XRuR1j6VS93fd4t+RNfVp/a7yvaZOR\n\
f0aSWYK4qZy7gzys1KH7akQON+LCpw6RTiIWimAzAZ2Yx8DMxbSzH4PYMQ7URI7/\n\
MRUPXz3qCwbubtkJwNNbFb+x8d87HR7GpLJMrt2MqboQBILTaaFYu3nvwi5RLVdZ\n\
h+wzEQbWDjR5RZo9SElhN9vJfKhSS2aYL8zpGhHb5e+IbYw5pzKgKLa6jnyLHqAz\n\
Jf5Dt4CqYJDzTpsBG5dH3d/f5isMBe2u+E5D901IG1v8eUKP1lEJrljqx9xpgYf0\n\
MtwwCn5dnom8WOpQvP9Im4Xdy7vZ7PIcsvuZeaJNsQIDAQABo38wfTAOBgNVHQ8B\n\
Af8EBAMCBaAwHQYDVR0lBBYwFAYIKwYBBQUHAwEGCCsGAQUFBwMCMAkGA1UdEwQC\n\
MAAwIgYDVR0RBBswGYYXc3BpZmZlOi8vbG9jYWxob3N0L3Rlc3QwHQYDVR0OBBYE\n\
FPWyhgkS+mDZTVK+kcRAHK1CSwyxMA0GCSqGSIb3DQEBCwUAA4IBAQDQwoTbmFB7\n\
xtfk2ieQAaul+AgCNopkr36xtE07vxEP307tC6hO2RMJUWYOFeioxPBbDpa5ff/3\n\
6n4QgHpnFAGDIvwvuUa1upIkvaHFYFlyPFvcyzBZqhob/wIn8WIITFfkzygbkxGi\n\
XzjpK0rIywC6cdaqYMDcIUyqNCO2l2FvccN7flo2pnppj6w55kv+FTX0C+AUv3qC\n\
p2OFoxDKsFWk52J0qXR/QefV5fFnrOLgqI2zCbyxSr7EZzGW9Fbr+YrpzXfI8Z0b\n\
8GGRaPE6WbPGjvc97Uwmp3T+4UkJatFnaAHnTsRikdbZ1F0xNcvE13pltbG3vFk0\n\
lQluKI5/n4db\n\
-----END CERTIFICATE-----";

    const TEST_KEY_PEM: &str = "-----BEGIN PRIVATE KEY-----\n\
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQDWfSLWEw9Kgfsn\n\
5dG5HWPpVL3d93i35E19Wn9rvK9pk5F/RpJZgripnLuDPKzUoftqRA434sKnDpFO\n\
IhaKYDMBnZjHwMzFtLMfg9gxDtREjv8xFQ9fPeoLBu5u2QnA01sVv7Hx3zsdHsak\n\
skyu3YypuhAEgtNpoVi7ee/CLlEtV1mH7DMRBtYONHlFmj1ISWE328l8qFJLZpgv\n\
zOkaEdvl74htjDmnMqAotrqOfIseoDMl/kO3gKpgkPNOmwEbl0fd39/mKwwF7a74\n\
TkP3TUgbW/x5Qo/WUQmuWOrH3GmBh/Qy3DAKfl2eibxY6lC8/0ibhd3Lu9ns8hyy\n\
+5l5ok2xAgMBAAECggEAAO0baGc+qqizB/ITHMSGuOw3waye5dRjjUYFxNZUv5T2\n\
jOEmIqLQ31Kg8KkjaeulJUlT8mPVSVljwT2ecUyHC9u9XCd1+uiT2W/9UADrY7xm\n\
V7TqkxO2XgPSpcHkK+P9wbNJNm0rWS3X18A5Wov0XotCJHLYLN2Yf37ATUtb6GE1\n\
J5wqaSaqVwLbhNk0rRojsWNO61LYYsEL3fA/Q2UA0lLfo5BkuHIHRJJvdtmpWX2L\n\
Rf6lV4nxdx+nxPIkqYo0wFLanuM+6+zO2ej094/Op3CWnxqXoUnCzyA8tut7+0zk\n\
o1LN5ygAdDFlJ0qvyPUTeDHLG+H0DfMKcI3jBRUmAQKBgQD56BH/+qH0A9oISwgM\n\
75C+mKt/88LFA5ztUOwz7k4opVOYtrUxDNKRqplI4bUedJMWUbm2kXFh00YIBt7u\n\
9PMgkQwq6j5IK4JzcPYto/Zl6bNuoiL7/WQU3lSTspu0xhEqAYC+KAxEI0WuuIVZ\n\
J9QSq1884dTBwHiXmnNmCX3BkQKBgQDbt/yOKjnsSJd5YtktWrJ9DnPamkwIqub1\n\
D59k/HwKs8StSHNFW0fkVpTRTa7R12CMgu1n5KvGOt2PX1VNPHh4O/8th1pkt2Jj\n\
lf29NMmSXcOi7KPjj0zBWmDAx0cgkt7ftQcc42+9CWxyUdbgYqMismaUit0zZkhR\n\
5nvsALm6IQKBgDoZHbYpCmW0T4gGCYUYXMoyrAw/G1S6Fk2FtqQMDtecN+cU8uLI\n\
XFvJEYHEF1tRNrDFpysufPGFMI7FKibbg3pavj1r37bfhqBX7qOFrs7amgBqaT+0\n\
FQRU+8yqhVBti6f8WXXb0Z41pQmNlFK506/Tb3yz88ZnfKGiIpniMv5BAoGAQn7K\n\
JlRNN184yHnL9FfwkLxg/5WW0UC3qQ7TVIK9H5gMO80jZagcd9RkMXvrHoKqK5ws\n\
MTcZbWK/TvaxIDDe3LR7o9HE35pIYo8wPaTOJEfQP2ySpPnnZtTtVyp4MjmAzf9B\n\
adLDLFi/w1FVUI9Jg+St+uKT00xvMqoocuI9U0ECgYEAzlapqhd+CXpy7KQKNtRt\n\
A/lJGE6bkB2JNXbr01DthVr5JSDPz39AxTRB9VeRUt5irB8f7OvmS7fy6+FY9Jxn\n\
QBAx6pG1tAXOEZt4R56+FIKBFcHJFB0ja/RQDRDLCZl+KFUDfgRNvomZx1lWBicI\n\
fPfrHw1nYcPliVB4Zbv8d1w=\n\
-----END PRIVATE KEY-----";

    fn get_test_svid() -> X509Svid {
        let cert_der = pem::parse(TEST_CERT_PEM).unwrap().contents().to_vec();
        let key_der = pem::parse(TEST_KEY_PEM).unwrap().contents().to_vec();
        X509Svid::parse_from_der(&cert_der, &key_der).expect("Failed to parse SVID")
    }

    fn get_test_bundle() -> X509Bundle {
        let cert_der = pem::parse(TEST_CERT_PEM).unwrap().contents().to_vec();
        let td = TrustDomain::new("localhost").unwrap();
        X509Bundle::parse_from_der(td, &cert_der).expect("Failed to parse Bundle")
    }

    #[test]
    fn test_write_svid_to_files_success() {
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

    #[test]
    fn test_write_bundle_to_file_success() {
        let temp_dir = TempDir::new().unwrap();
        let cert_dir = temp_dir.path();
        let bundle = get_test_bundle();

        write_bundle_to_file(&bundle, cert_dir, "bundle.pem").unwrap();

        assert!(cert_dir.join("bundle.pem").exists());
        let content = fs::read_to_string(cert_dir.join("bundle.pem")).unwrap();
        assert!(content.contains("BEGIN CERTIFICATE"));
    }

    #[test]
    fn test_pem_encoding_logic() {
        let data = vec![0x30, 0x01, 0x01];
        let pem = pem::encode(&pem::Pem::new("CERTIFICATE", data));
        assert!(pem.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(pem.contains("-----END CERTIFICATE-----"));
    }
}
