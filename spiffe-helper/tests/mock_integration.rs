use anyhow::Result;
use rcgen::{BasicConstraints, Certificate, CertificateParams, DistinguishedName, IsCa};
use spiffe_helper_rust::cli::config::Config;
use spiffe_helper_rust::oneshot;
use std::process::{Command, Stdio};
use std::time::Duration;
use tempfile::TempDir;
use time::{Duration as TimeDuration, OffsetDateTime};
use tokio::time::sleep;

// Helper to generate a self-signed cert and key
fn generate_cert() -> Result<(String, String, String)> {
    // 1. Generate CA
    let mut ca_params = CertificateParams::default();
    ca_params.not_before = OffsetDateTime::now_utc() - TimeDuration::days(1);
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.distinguished_name = DistinguishedName::new();
    ca_params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "example.org");
    ca_params
        .key_usages
        .push(rcgen::KeyUsagePurpose::KeyCertSign);
    ca_params.key_usages.push(rcgen::KeyUsagePurpose::CrlSign);

    let ca_cert = Certificate::from_params(ca_params)?;
    let ca_pem = ca_cert.serialize_pem()?;

    // 2. Generate Leaf
    let mut leaf_params = CertificateParams::default();
    leaf_params.not_before = OffsetDateTime::now_utc() - TimeDuration::days(1);
    leaf_params.is_ca = IsCa::NoCa;
    leaf_params.distinguished_name = DistinguishedName::new();
    leaf_params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "myservice");
    leaf_params.subject_alt_names.push(rcgen::SanType::URI(
        "spiffe://example.org/myservice".to_string(),
    ));
    leaf_params
        .key_usages
        .push(rcgen::KeyUsagePurpose::DigitalSignature);
    leaf_params
        .key_usages
        .push(rcgen::KeyUsagePurpose::KeyEncipherment);
    leaf_params
        .extended_key_usages
        .push(rcgen::ExtendedKeyUsagePurpose::ServerAuth);
    leaf_params
        .extended_key_usages
        .push(rcgen::ExtendedKeyUsagePurpose::ClientAuth);

    // Add Basic Constraints: cA=FALSE
    leaf_params
        .custom_extensions
        .push(rcgen::CustomExtension::from_oid_content(
            &[2, 5, 29, 19],
            vec![0x30, 0x00],
        ));

    let leaf_cert = Certificate::from_params(leaf_params)?;
    let leaf_cert_signed = leaf_cert.serialize_pem_with_signer(&ca_cert)?;
    let leaf_key = leaf_cert.serialize_private_key_pem();

    Ok((leaf_cert_signed, leaf_key, ca_pem))
}

#[tokio::test]
async fn test_oneshot_with_mock_agent() -> Result<()> {
    // Init tracing
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();

    // 1. Setup temp directory
    let temp_dir = TempDir::new()?;
    let cert_path = temp_dir.path().join("svid.pem");
    let key_path = temp_dir.path().join("key.pem");
    let bundle_path = temp_dir.path().join("bundle.pem");
    let socket_path = temp_dir.path().join("agent.sock");

    let output_cert_path = temp_dir.path().join("out_svid.pem");
    let output_key_path = temp_dir.path().join("out_key.pem");
    let output_bundle_path = temp_dir.path().join("out_bundle.pem");

    // 2. Generate Certs
    let (cert, key, bundle) = generate_cert()?;
    std::fs::write(&cert_path, &cert)?;
    std::fs::write(&key_path, &key)?;
    std::fs::write(&bundle_path, &bundle)?;

    // 3. Build spire-mock (ensure it's fresh)
    // We assume the test runs in the spiffe-helper directory, but cargo might run from root.
    // Let's use cargo build at workspace root.
    // To find workspace root, we can check parent of current dir if we are in spiffe-helper.
    let current_dir = std::env::current_dir()?;
    let workspace_root = if current_dir.ends_with("spiffe-helper") {
        current_dir.parent().unwrap().to_path_buf()
    } else {
        current_dir.clone()
    };

    let status = Command::new("cargo")
        .current_dir(&workspace_root)
        .args(["build", "-p", "spire-mock"])
        .status()?;
    assert!(status.success(), "Failed to build spire-mock");

    // 4. Start spire-mock
    // Binary location: workspace_root/target/debug/spire-mock
    let mock_bin = workspace_root.join("target/debug/spire-mock");

    let mut mock_process = Command::new(mock_bin)
        .arg("--socket-path")
        .arg(&socket_path)
        .arg("--cert-path")
        .arg(&cert_path)
        .arg("--key-path")
        .arg(&key_path)
        .arg("--bundle-path")
        .arg(&bundle_path)
        .arg("--spiffe-id")
        .arg("spiffe://example.org/myservice")
        .env("RUST_LOG", "debug")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Wait for socket
    let mut retries = 0;
    while !socket_path.exists() {
        if retries > 50 {
            // 5 seconds
            mock_process.kill()?;
            panic!(
                "Mock agent failed to start. Socket not found at {:?}",
                socket_path
            );
        }
        sleep(Duration::from_millis(100)).await;
        retries += 1;
    }

    // 5. Run spiffe-helper oneshot
    let config = Config {
        agent_address: Some(format!("unix://{}", socket_path.to_string_lossy())),
        cert_dir: Some(temp_dir.path().to_string_lossy().to_string()),
        svid_file_name: Some("out_svid.pem".to_string()),
        svid_key_file_name: Some("out_key.pem".to_string()),
        svid_bundle_file_name: Some("out_bundle.pem".to_string()),
        daemon_mode: Some(false),
        ..Default::default()
    };

    // run() takes config by value
    let result = oneshot::run(config).await;

    // Kill mock before asserting to ensure cleanup
    mock_process.kill()?;
    let output = mock_process.wait_with_output()?;

    if result.is_err() {
        println!("Mock stdout: {}", String::from_utf8_lossy(&output.stdout));
        println!("Mock stderr: {}", String::from_utf8_lossy(&output.stderr));
    }

    assert!(result.is_ok(), "Oneshot run failed: {:?}", result.err());

    // 6. Verify outputs
    assert!(output_cert_path.exists(), "Output cert not found");
    assert!(output_key_path.exists(), "Output key not found");
    assert!(output_bundle_path.exists(), "Output bundle not found");

    let saved_cert = std::fs::read_to_string(output_cert_path)?;
    // It should contain the certificate content (PEM)
    assert!(saved_cert.contains("BEGIN CERTIFICATE"));

    Ok(())
}

#[tokio::test]
async fn test_daemon_with_mock_agent() -> Result<()> {
    use spiffe_helper_rust::daemon;

    // 1. Setup temp directory
    let temp_dir = TempDir::new()?;
    let cert_path = temp_dir.path().join("svid.pem");
    let key_path = temp_dir.path().join("key.pem");
    let bundle_path = temp_dir.path().join("bundle.pem");
    let socket_path = temp_dir.path().join("agent_daemon.sock");

    let output_cert_path = temp_dir.path().join("out_svid.pem");
    let output_key_path = temp_dir.path().join("out_key.pem");
    let output_bundle_path = temp_dir.path().join("out_bundle.pem");

    // 2. Generate Certs
    let (cert, key, bundle) = generate_cert()?;
    std::fs::write(&cert_path, &cert)?;
    std::fs::write(&key_path, &key)?;
    std::fs::write(&bundle_path, &bundle)?;

    // 3. Start spire-mock
    let current_dir = std::env::current_dir()?;
    let workspace_root = if current_dir.ends_with("spiffe-helper") {
        current_dir.parent().unwrap().to_path_buf()
    } else {
        current_dir.clone()
    };
    let mock_bin = workspace_root.join("target/debug/spire-mock");

    let mut mock_process = Command::new(mock_bin)
        .arg("--socket-path")
        .arg(&socket_path)
        .arg("--cert-path")
        .arg(&cert_path)
        .arg("--key-path")
        .arg(&key_path)
        .arg("--bundle-path")
        .arg(&bundle_path)
        .arg("--spiffe-id")
        .arg("spiffe://example.org/myservice")
        .env("RUST_LOG", "debug")
        .spawn()?;

    // Wait for socket
    let mut retries = 0;
    while !socket_path.exists() {
        if retries > 50 {
            mock_process.kill()?;
            panic!("Mock agent failed to start");
        }
        sleep(Duration::from_millis(100)).await;
        retries += 1;
    }

    // 4. Run spiffe-helper daemon in background
    let config = Config {
        agent_address: Some(format!("unix://{}", socket_path.to_string_lossy())),
        cert_dir: Some(temp_dir.path().to_string_lossy().to_string()),
        svid_file_name: Some("out_svid.pem".to_string()),
        svid_key_file_name: Some("out_key.pem".to_string()),
        svid_bundle_file_name: Some("out_bundle.pem".to_string()),
        daemon_mode: Some(true),
        ..Default::default()
    };

    // We need to run the daemon in a way that we can stop it
    // Since daemon::run() is an infinite loop (until SIGTERM), we spawn it.
    let daemon_config = config.clone();
    let daemon_handle = tokio::spawn(async move { daemon::run(daemon_config).await });

    // 5. Wait for initial fetch
    let mut retries = 0;
    while !output_cert_path.exists() {
        if retries > 50 {
            break;
        }
        sleep(Duration::from_millis(100)).await;
        retries += 1;
    }

    assert!(
        output_cert_path.exists(),
        "Daemon failed to fetch initial cert"
    );
    assert!(
        output_key_path.exists(),
        "Daemon failed to fetch initial key"
    );
    assert!(
        output_bundle_path.exists(),
        "Daemon failed to fetch initial bundle"
    );

    // 6. Cleanup
    // In a real test we'd send SIGTERM to the daemon if it was a separate process.
    // Since it's a tokio task, we can just abort it if it doesn't have its own signal handling that we want to test.
    daemon_handle.abort();
    mock_process.kill()?;

    Ok(())
}
