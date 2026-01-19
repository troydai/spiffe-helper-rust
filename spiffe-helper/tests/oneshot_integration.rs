use spiffe_helper::cli::Config;
use spiffe_helper::{oneshot, workload_api};
use tempfile::TempDir;

mod common;

const DEFAULT_ROTATION_SECONDS: u32 = 300;

/// Test that one-shot mode successfully fetches and writes X.509 certificates
/// from the mock SPIRE agent.
#[tokio::test(flavor = "multi_thread")]
async fn test_oneshot_writes_cert_and_key() {
    // Create temporary directories for socket and certificates
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let socket_path = temp_dir.path().join("agent.sock");
    let cert_dir = temp_dir.path().join("certs");

    // Start mock SPIRE agent in the background
    let socket_path_clone = socket_path.clone();
    let server_handle = tokio::spawn(async move {
        common::start_mock_agent(&socket_path_clone, DEFAULT_ROTATION_SECONDS).await;
    });

    // Wait for the socket to be ready
    common::assert_socket_ready(&socket_path).await;

    // Configure spiffe-helper for one-shot mode
    let agent_address = format!("unix://{}", socket_path.display());
    let config = Config {
        agent_address: Some(agent_address.clone()),
        cert_dir: Some(cert_dir.to_str().unwrap().to_string()),
        daemon_mode: Some(false),
        svid_file_name: Some("svid.pem".to_string()),
        svid_key_file_name: Some("svid_key.pem".to_string()),
        ..Default::default()
    };

    // Run one-shot mode
    let source = workload_api::create_x509_source(&agent_address)
        .await
        .expect("Failed to create X509Source");
    let result = oneshot::run(source, config).await;
    assert!(result.is_ok(), "One-shot mode failed: {:?}", result.err());

    // Verify certificate files were written
    let cert_path = cert_dir.join("svid.pem");
    let key_path = cert_dir.join("svid_key.pem");
    let _cer_bytes = common::assert_x509_cert(&cert_path);
    let _key_bytes = common::assert_x509_key(&key_path);

    // Abort the server task
    server_handle.abort();
}

/// Test that one-shot mode works with custom file names
#[tokio::test]
async fn test_oneshot_creates_dir_and_custom_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let socket_path = temp_dir.path().join("agent.sock");
    let cert_dir = temp_dir.path().join("nested").join("certs").join("dir");

    // Ensure the nested directory doesn't exist
    assert!(!cert_dir.exists());

    let socket_path_clone = socket_path.clone();
    let server_handle = tokio::spawn(async move {
        common::start_mock_agent(&socket_path_clone, DEFAULT_ROTATION_SECONDS).await;
    });

    common::assert_socket_ready(&socket_path).await;

    let agent_address = format!("unix://{}", socket_path.display());
    let config = Config {
        agent_address: Some(agent_address.clone()),
        cert_dir: Some(cert_dir.to_str().unwrap().to_string()),
        daemon_mode: Some(false),
        svid_file_name: Some("custom_cert.pem".to_string()),
        svid_key_file_name: Some("custom_key.pem".to_string()),
        ..Default::default()
    };

    let source = workload_api::create_x509_source(&agent_address)
        .await
        .expect("Failed to create X509Source");
    let result = oneshot::run(source, config).await;
    assert!(result.is_ok(), "One-shot mode failed: {:?}", result.err());

    // Verify directory and custom file names
    assert!(cert_dir.exists(), "Certificate directory was not created");
    let cert_path = cert_dir.join("custom_cert.pem");
    let _cert_serial = common::assert_x509_cert(&cert_path);
    let key_path = cert_dir.join("custom_key.pem");
    let _key_bytes = common::assert_x509_key(&key_path);

    server_handle.abort();
}
