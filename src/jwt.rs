use anyhow::{Context, Result};
use spiffe::svid::jwt::JwtSvid as SpiffeJwtSvid;
use spiffe::workload_api::client::WorkloadApiClient;

use crate::config::JwtSvid as JwtSvidConfig;

/// Fetch a JWT SVID for the specified audience configuration
pub async fn fetch_jwt_svid_for_config(
    agent_address: &str,
    jwt_config: &JwtSvidConfig,
) -> Result<SpiffeJwtSvid> {
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

    // Combine primary audience with extra audiences
    let mut audiences = vec![jwt_config.jwt_audience.clone()];
    if let Some(ref extra) = jwt_config.jwt_extra_audiences {
        audiences.extend(extra.iter().cloned());
    }

    println!("Fetching JWT SVID for audiences: {:?}", audiences);

    // Fetch JWT SVID with retries (workload may need time to attest)
    let mut jwt_svid = None;
    let mut last_error_msg = None;
    for attempt in 1..=10 {
        match client.fetch_jwt_svid(&audiences, None).await {
            Ok(svid) => {
                jwt_svid = Some(svid);
                if attempt > 1 {
                    eprintln!("Successfully fetched JWT SVID after {attempt} attempts");
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
                    "Failed to fetch JWT SVID from SPIRE agent after {} attempts: {}",
                    attempt,
                    last_error_msg.as_ref().unwrap_or(&format!("{e:?}"))
                ));
            }
        }
    }

    let jwt_svid = jwt_svid.ok_or_else(|| {
        anyhow::anyhow!(
            "Failed to fetch JWT SVID from SPIRE agent: {}",
            last_error_msg.unwrap_or_else(|| "unknown error".to_string())
        )
    })?;

    println!("Fetched JWT SVID with SPIFFE ID: {}", jwt_svid.spiffe_id());

    Ok(jwt_svid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_jwt_svid_missing_agent_address() {
        // Test with invalid agent address
        let config = JwtSvidConfig {
            jwt_audience: "test-audience".to_string(),
            jwt_extra_audiences: None,
            jwt_svid_file_name: "test.jwt".to_string(),
        };

        let result = fetch_jwt_svid_for_config("", &config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_jwt_svid_nonexistent_socket() {
        // Test with non-existent socket (should fail after retries)
        let config = JwtSvidConfig {
            jwt_audience: "test-audience".to_string(),
            jwt_extra_audiences: None,
            jwt_svid_file_name: "test.jwt".to_string(),
        };

        let result =
            fetch_jwt_svid_for_config("unix:///tmp/nonexistent-socket-12345.sock", &config).await;
        assert!(result.is_err());
        // Should fail on connection or fetching, both are acceptable
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Failed to fetch JWT SVID")
                || error_msg.contains("Failed to connect")
                || error_msg.contains("No such file")
        );
    }

    #[tokio::test]
    async fn test_fetch_jwt_svid_with_extra_audiences() {
        // Test that extra audiences are included
        let config = JwtSvidConfig {
            jwt_audience: "primary".to_string(),
            jwt_extra_audiences: Some(vec!["extra1".to_string(), "extra2".to_string()]),
            jwt_svid_file_name: "test.jwt".to_string(),
        };

        // This will fail to connect, but we can verify the function accepts the config
        let result =
            fetch_jwt_svid_for_config("unix:///tmp/nonexistent-socket-12345.sock", &config).await;
        assert!(result.is_err());
        // Should fail on connection, not on config parsing
        let error_msg = result.unwrap_err().to_string();
        assert!(!error_msg.contains("jwt_audience"));
        assert!(!error_msg.contains("jwt_extra_audiences"));
    }
}
