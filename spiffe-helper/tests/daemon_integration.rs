use spiffe_helper::cli::Config;
use spiffe_helper::daemon;
use spiffe_helper::signal;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

mod common;

/// Test that daemon mode rotates certificates, then shuts down on SIGTERM.
#[tokio::test(flavor = "multi_thread")]
async fn test_daemon_rotates_certs_then_shutdown() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let socket_path = temp_dir.path().join("agent.sock");
    let cert_dir = temp_dir.path().join("certs");

    let socket_path_clone = socket_path.clone();
    let server_handle = tokio::spawn(async move {
        common::start_mock_agent(&socket_path_clone, 1).await;
    });

    common::assert_socket_ready(&socket_path).await;

    let config = Config {
        agent_address: Some(format!("unix://{}", socket_path.display())),
        cert_dir: Some(cert_dir.to_str().unwrap().to_string()),
        daemon_mode: Some(true),
        svid_file_name: Some("svid.pem".to_string()),
        svid_key_file_name: Some("svid_key.pem".to_string()),
        ..Default::default()
    };

    let daemon_handle = tokio::spawn(async move { daemon::run(config).await });

    let cert_path = cert_dir.join("svid.pem");
    let key_path = cert_dir.join("svid_key.pem");

    assert_file_exists(&cert_path).await;
    assert_file_exists(&key_path).await;

    let initial_serial = common::assert_x509_cert(&cert_path);
    let initial_key = common::assert_x509_key(&key_path);

    let rotated_cert = assert_cert_rotated_by_serial(&cert_path, &initial_serial).await;
    let rotated_key = assert_key_rotated_by_bytes(&key_path, &initial_key).await;

    let (rotated_serial, rotated_not_before, rotated_not_after) = cert_metadata(&rotated_cert);

    assert_ne!(
        initial_serial, rotated_serial,
        "Certificate serial did not rotate"
    );
    assert!(
        rotated_not_before < rotated_not_after,
        "Rotated certificate validity window is invalid"
    );
    assert!(
        common::is_time_within_leeway(rotated_not_before, rotated_not_after),
        "Rotated certificate is not currently valid"
    );
    assert_ne!(initial_key, rotated_key, "Private key did not rotate");

    let pid = nix::unistd::getpid();
    signal::send_signal(pid.as_raw(), signal::Signal::SIGTERM)
        .expect("Failed to send SIGTERM to daemon");

    let daemon_result = tokio::time::timeout(std::time::Duration::from_secs(5), daemon_handle)
        .await
        .expect("Daemon did not shut down within timeout");

    let run_result = daemon_result.expect("Daemon task panicked");
    assert!(
        run_result.is_ok(),
        "Daemon mode failed: {:?}",
        run_result.err()
    );

    server_handle.abort();
}

/// Wait for a file to exist (with timeout).
async fn assert_file_exists(path: &Path) {
    let max_attempts = 50;
    let delay = std::time::Duration::from_millis(100);
    let mut exists = false;

    for _ in 0..max_attempts {
        if path.exists() {
            exists = true;
            break;
        }
        tokio::time::sleep(delay).await;
    }

    assert!(
        exists,
        "File was not created within timeout: {}",
        path.display()
    );
}

/// Wait for a cert's serial to change (with timeout).
async fn assert_cert_rotated_by_serial(path: &Path, initial_serial: &[u8]) -> String {
    let max_attempts = 80;
    let delay = std::time::Duration::from_millis(100);
    let mut rotated = None;

    for _ in 0..max_attempts {
        if let Ok(current) = fs::read_to_string(path) {
            if let Some(serial) = cert_serial_bytes(&current) {
                if serial != initial_serial {
                    rotated = Some(current);
                    break;
                }
            }
        }
        tokio::time::sleep(delay).await;
    }

    assert!(
        rotated.is_some(),
        "Certificate did not rotate within timeout: {}",
        path.display()
    );

    rotated.expect("Rotation must be present after assert")
}

/// Wait for a key's contents to change (with timeout).
async fn assert_key_rotated_by_bytes(path: &Path, initial_key: &[u8]) -> Vec<u8> {
    let max_attempts = 80;
    let delay = std::time::Duration::from_millis(100);
    let mut rotated = None;

    for _ in 0..max_attempts {
        if let Ok(current) = fs::read_to_string(path) {
            if let Some(key_bytes) = key_bytes(&current) {
                if key_bytes != initial_key {
                    rotated = Some(key_bytes);
                    break;
                }
            }
        }
        tokio::time::sleep(delay).await;
    }

    assert!(
        rotated.is_some(),
        "Private key did not rotate within timeout: {}",
        path.display()
    );

    rotated.expect("Rotation must be present after assert")
}

fn cert_metadata(
    pem: &str,
) -> (
    Vec<u8>,
    x509_parser::time::ASN1Time,
    x509_parser::time::ASN1Time,
) {
    let pem_data = pem::parse(pem).expect("Failed to parse certificate PEM");
    let (_, cert) =
        x509_parser::parse_x509_certificate(&pem_data.contents).expect("Failed to parse X.509");

    let serial = cert.tbs_certificate.serial.to_bytes_be();
    let not_before = cert.tbs_certificate.validity.not_before;
    let not_after = cert.tbs_certificate.validity.not_after;

    (serial, not_before, not_after)
}

fn cert_serial_bytes(pem: &str) -> Option<Vec<u8>> {
    let pem_data = pem::parse(pem).ok()?;
    let (_, cert) = x509_parser::parse_x509_certificate(&pem_data.contents).ok()?;
    Some(cert.tbs_certificate.serial.to_bytes_be())
}

fn key_bytes(pem: &str) -> Option<Vec<u8>> {
    let pem_data = pem::parse(pem).ok()?;
    Some(pem_data.contents)
}
