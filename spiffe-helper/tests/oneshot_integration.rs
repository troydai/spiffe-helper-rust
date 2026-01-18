use spiffe_helper::cli::Config;
use spiffe_helper::oneshot;
use spire_agent_mock::server::{MockWorkloadApi, SpiffeWorkloadApiServer};
use spire_agent_mock::svid::SvidConfig;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;

/// Test that one-shot mode successfully fetches and writes X.509 certificates
/// from the mock SPIRE agent.
#[tokio::test(flavor = "multi_thread")]
async fn test_oneshot_mode_fetches_and_writes_certificates() {
    // Create temporary directories for socket and certificates
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let socket_path = temp_dir.path().join("agent.sock");
    let cert_dir = temp_dir.path().join("certs");

    // Start mock SPIRE agent in the background
    let socket_path_clone = socket_path.clone();
    let server_handle = tokio::spawn(async move {
        start_mock_agent(&socket_path_clone).await;
    });

    // Wait for the socket to be ready
    wait_for_socket(&socket_path).await;

    // Configure spiffe-helper for one-shot mode
    let config = Config {
        agent_address: Some(format!("unix://{}", socket_path.display())),
        cert_dir: Some(cert_dir.to_str().unwrap().to_string()),
        daemon_mode: Some(false),
        svid_file_name: Some("svid.pem".to_string()),
        svid_key_file_name: Some("svid_key.pem".to_string()),
        ..Default::default()
    };

    // Run one-shot mode
    let result = oneshot::run(config).await;
    assert!(result.is_ok(), "One-shot mode failed: {:?}", result.err());

    // Verify certificate files were written
    let cert_path = cert_dir.join("svid.pem");
    let key_path = cert_dir.join("svid_key.pem");

    assert!(cert_path.exists(), "Certificate file was not created");
    assert!(key_path.exists(), "Private key file was not created");

    // Verify file contents
    let cert_content = fs::read_to_string(&cert_path).expect("Failed to read certificate");
    let key_content = fs::read_to_string(&key_path).expect("Failed to read private key");

    assert!(
        cert_content.contains("-----BEGIN CERTIFICATE-----"),
        "Certificate does not contain PEM header"
    );
    assert!(
        cert_content.contains("-----END CERTIFICATE-----"),
        "Certificate does not contain PEM footer"
    );
    assert!(
        key_content.contains("-----BEGIN PRIVATE KEY-----"),
        "Private key does not contain PEM header"
    );
    assert!(
        key_content.contains("-----END PRIVATE KEY-----"),
        "Private key does not contain PEM footer"
    );

    // Abort the server task
    server_handle.abort();
}

// /// Test that one-shot mode works with custom file names
// #[tokio::test]
// async fn test_oneshot_mode_with_custom_file_names() {
//     let temp_dir = TempDir::new().expect("Failed to create temp dir");
//     let socket_path = temp_dir.path().join("agent.sock");
//     let cert_dir = temp_dir.path().join("certs");

//     let socket_path_clone = socket_path.clone();
//     let server_handle = tokio::spawn(async move {
//         start_mock_agent(&socket_path_clone).await;
//     });

//     wait_for_socket(&socket_path).await;

//     let config = Config {
//         agent_address: Some(format!("unix://{}", socket_path.display())),
//         cert_dir: Some(cert_dir.to_str().unwrap().to_string()),
//         daemon_mode: Some(false),
//         svid_file_name: Some("custom_cert.pem".to_string()),
//         svid_key_file_name: Some("custom_key.pem".to_string()),
//         ..Default::default()
//     };

//     let result = oneshot::run(config).await;
//     assert!(result.is_ok(), "One-shot mode failed: {:?}", result.err());

//     // Verify custom file names
//     assert!(
//         cert_dir.join("custom_cert.pem").exists(),
//         "Custom certificate file was not created"
//     );
//     assert!(
//         cert_dir.join("custom_key.pem").exists(),
//         "Custom key file was not created"
//     );

//     server_handle.abort();
// }

// /// Test that one-shot mode creates the certificate directory if it doesn't exist
// #[tokio::test]
// async fn test_oneshot_mode_creates_cert_directory() {
//     let temp_dir = TempDir::new().expect("Failed to create temp dir");
//     let socket_path = temp_dir.path().join("agent.sock");
//     let cert_dir = temp_dir.path().join("nested").join("certs").join("dir");

//     // Ensure the nested directory doesn't exist
//     assert!(!cert_dir.exists());

//     let socket_path_clone = socket_path.clone();
//     let server_handle = tokio::spawn(async move {
//         start_mock_agent(&socket_path_clone).await;
//     });

//     wait_for_socket(&socket_path).await;

//     let config = Config {
//         agent_address: Some(format!("unix://{}", socket_path.display())),
//         cert_dir: Some(cert_dir.to_str().unwrap().to_string()),
//         daemon_mode: Some(false),
//         ..Default::default()
//     };

//     let result = oneshot::run(config).await;
//     assert!(result.is_ok(), "One-shot mode failed: {:?}", result.err());

//     // Verify directory and files were created
//     assert!(cert_dir.exists(), "Certificate directory was not created");
//     assert!(
//         cert_dir.join("svid.pem").exists(),
//         "Certificate file was not created"
//     );
//     assert!(
//         cert_dir.join("svid_key.pem").exists(),
//         "Private key file was not created"
//     );

//     server_handle.abort();
// }

// /// Test that certificates contain valid SPIFFE ID
// #[tokio::test]
// async fn test_oneshot_mode_certificate_contains_spiffe_id() {
//     let temp_dir = TempDir::new().expect("Failed to create temp dir");
//     let socket_path = temp_dir.path().join("agent.sock");
//     let cert_dir = temp_dir.path().join("certs");

//     let socket_path_clone = socket_path.clone();
//     let server_handle = tokio::spawn(async move {
//         start_mock_agent(&socket_path_clone).await;
//     });

//     wait_for_socket(&socket_path).await;

//     let config = Config {
//         agent_address: Some(format!("unix://{}", socket_path.display())),
//         cert_dir: Some(cert_dir.to_str().unwrap().to_string()),
//         daemon_mode: Some(false),
//         ..Default::default()
//     };

//     let result = oneshot::run(config).await;
//     assert!(result.is_ok(), "One-shot mode failed: {:?}", result.err());

//     // Read and parse the certificate to verify SPIFFE ID
//     let cert_pem = fs::read_to_string(cert_dir.join("svid.pem")).unwrap();
//     let pem_data = pem::parse(&cert_pem).expect("Failed to parse certificate PEM");

//     // Parse the certificate
//     let (_, cert) =
//         x509_parser::parse_x509_certificate(&pem_data.contents).expect("Failed to parse X.509");

//     // Check for SPIFFE ID in Subject Alternative Names
//     let san_ext = cert
//         .subject_alternative_name()
//         .expect("Failed to get SAN extension")
//         .expect("No SAN extension found");

//     let has_spiffe_uri = san_ext.value.general_names.iter().any(|name| {
//         if let x509_parser::prelude::GeneralName::URI(uri) = name {
//             uri.starts_with("spiffe://")
//         } else {
//             false
//         }
//     });

//     assert!(has_spiffe_uri, "Certificate does not contain SPIFFE URI");

//     server_handle.abort();
// }

/// Start the mock SPIRE agent on the given socket path
async fn start_mock_agent(socket_path: &PathBuf) {
    // Remove existing socket if it exists
    if socket_path.exists() {
        fs::remove_file(socket_path).unwrap();
    }

    // Create parent directory if needed
    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }

    let uds = UnixListener::bind(socket_path).expect("Failed to bind to socket");
    let uds_stream = UnixListenerStream::new(uds);

    let config = SvidConfig {
        trust_domain: "example.org".to_string(),
        workload_path: "/test/workload".to_string(),
        ttl_seconds: 300,
    };
    let service = MockWorkloadApi::with_config(config);

    println!("starting mock agent at {:?}", socket_path);

    Server::builder()
        .add_service(SpiffeWorkloadApiServer::new(service))
        .serve_with_incoming(uds_stream)
        .await
        .unwrap();
}

/// Wait for the socket file to exist (with timeout)
async fn wait_for_socket(socket_path: &Path) {
    let max_attempts = 50;
    let delay = std::time::Duration::from_millis(100);

    for _ in 0..max_attempts {
        if socket_path.exists() {
            // Give the server a moment to fully start accepting connections
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            return;
        }
        tokio::time::sleep(delay).await;
    }

    panic!(
        "Socket file was not created within timeout: {}",
        socket_path.display()
    );
}
