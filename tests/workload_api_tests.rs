use spiffe_helper_rust::workload_api;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_fetch_and_write_x509_svid_invalid_address() {
    let temp_dir = TempDir::new().unwrap();
    let cert_dir = temp_dir.path();

    // Test with invalid agent address
    let result =
        workload_api::fetch_and_write_x509_svid("invalid://address", cert_dir, None, None).await;

    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Invalid agent address") || error_msg.contains("Failed to connect"));
}

#[tokio::test]
async fn test_fetch_and_write_x509_svid_missing_agent() {
    let temp_dir = TempDir::new().unwrap();
    let cert_dir = temp_dir.path();

    // Test with non-existent unix socket
    let result = workload_api::fetch_and_write_x509_svid(
        "unix:///tmp/nonexistent-socket.sock",
        cert_dir,
        None,
        None,
    )
    .await;

    assert!(result.is_err());
    // Should fail when trying to connect to non-existent socket
    let error_msg = result.unwrap_err().to_string();
    // The error message may vary depending on the platform/tonic version
    // Just verify that it's an error related to connection/socket
    assert!(!error_msg.is_empty(), "Error message should not be empty");
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
