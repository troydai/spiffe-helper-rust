use anyhow::{Context, Result};
use spiffe::bundle::jwt::JwtBundleSet;
use spiffe::workload_api::client::WorkloadApiClient;

use crate::cli::Config;

/// Fetch JWT bundles from the Workload API
pub async fn fetch_jwt_bundles(agent_address: &str) -> Result<JwtBundleSet> {
    // Convert agent_address to the format expected by spiffe crate
    let spiffe_path = if agent_address.starts_with("unix://") {
        let socket_path = agent_address
            .strip_prefix("unix://")
            .ok_or_else(|| anyhow::anyhow!("Invalid unix socket address: {agent_address}"))?;
        // Use unix: prefix (not unix://) for spiffe crate
        format!("unix:{socket_path}")
    } else {
        agent_address.to_string()
    };

    // Create Workload API client
    let mut client = WorkloadApiClient::new_from_path(&spiffe_path)
        .await
        .with_context(|| format!("Failed to connect to SPIRE agent at {agent_address}"))?;

    println!("Fetching JWT bundles from {}", agent_address);

    // Fetch JWT bundles with retries (workload may need time to attest)
    let mut bundle_set = None;
    let mut last_error_msg = None;
    for attempt in 1..=10 {
        match client.fetch_jwt_bundles().await {
            Ok(bundles) => {
                bundle_set = Some(bundles);
                if attempt > 1 {
                    eprintln!("Successfully fetched JWT bundles after {attempt} attempts");
                }
                break;
            }
            Err(e) => {
                let error_str = format!("{e:?}");
                last_error_msg = Some(format!("{e} ({error_str})"));
                // If it's a permission denied error, the workload may still be attesting
                if error_str.contains("PermissionDenied") && attempt < 10 {
                    // Wait before retrying (exponential backoff: 1s, 2s, 4s, 8s, 16s, etc., max 16s)
                    let delay = std::cmp::min(1u64 << (attempt - 1), 16);
                    eprintln!(
                        "Attempt {attempt} failed (PermissionDenied), retrying in {delay}s..."
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                    continue;
                }
                // For other errors or last attempt, return the error
                return Err(anyhow::anyhow!(
                    "Failed to fetch JWT bundles from SPIRE agent after {} attempts: {}",
                    attempt,
                    last_error_msg.as_ref().unwrap_or(&format!("{e:?}"))
                ));
            }
        }
    }

    let bundle_set = bundle_set.ok_or_else(|| {
        anyhow::anyhow!(
            "Failed to fetch JWT bundles from SPIRE agent: {}",
            last_error_msg.unwrap_or_else(|| "unknown error".to_string())
        )
    })?;

    println!("Fetched JWT bundles successfully");

    Ok(bundle_set)
}

/// Check if JWT bundle fetching is enabled in configuration
pub fn should_fetch_jwt_bundle(config: &Config) -> bool {
    config.jwt_bundle_file_name.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_jwt_bundles_missing_agent_address() {
        // Test with invalid agent address
        let result = fetch_jwt_bundles("").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_jwt_bundles_nonexistent_socket() {
        // Test with non-existent socket (should fail after retries)
        let result = fetch_jwt_bundles("unix:///tmp/nonexistent-socket-12345.sock").await;
        assert!(result.is_err());
        // Should fail on connection or fetching, both are acceptable
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Failed to fetch JWT bundles")
                || error_msg.contains("Failed to connect")
                || error_msg.contains("No such file")
        );
    }

    #[test]
    fn test_should_fetch_jwt_bundle_enabled() {
        let config = Config {
            jwt_bundle_file_name: Some("bundle.json".to_string()),
            ..Default::default()
        };
        assert!(should_fetch_jwt_bundle(&config));
    }

    #[test]
    fn test_should_fetch_jwt_bundle_disabled() {
        let config = Config {
            jwt_bundle_file_name: None,
            ..Default::default()
        };
        assert!(!should_fetch_jwt_bundle(&config));
    }
}
