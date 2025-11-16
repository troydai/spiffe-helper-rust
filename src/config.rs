use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthChecks {
    pub listener_enabled: Option<bool>,
    pub bind_port: Option<u16>,
    pub liveness_path: Option<String>,
    pub readiness_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtSvid {
    pub jwt_audience: String,
    pub jwt_extra_audiences: Option<Vec<String>>,
    pub jwt_svid_file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub agent_address: Option<String>,
    pub cmd: Option<String>,
    pub cmd_args: Option<String>,
    pub pid_file_name: Option<String>,
    pub cert_dir: Option<String>,
    pub daemon_mode: Option<bool>,
    pub add_intermediates_to_bundle: Option<bool>,
    pub renew_signal: Option<String>,
    pub svid_file_name: Option<String>,
    pub svid_key_file_name: Option<String>,
    pub svid_bundle_file_name: Option<String>,
    pub jwt_svids: Option<Vec<JwtSvid>>,
    pub jwt_bundle_file_name: Option<String>,
    pub include_federated_domains: Option<bool>,
    pub cert_file_mode: Option<String>,
    pub key_file_mode: Option<String>,
    pub jwt_bundle_file_mode: Option<String>,
    pub jwt_svid_file_mode: Option<String>,
    pub hint: Option<String>,
    pub omit_expired: Option<bool>,
    pub health_checks: Option<HealthChecks>,
}

pub fn parse_hcl_config(path: &std::path::Path) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;

    let value: hcl::Value = hcl::from_str(&content)
        .with_context(|| format!("Failed to parse HCL config file: {}", path.display()))?;

    parse_hcl_value_to_config(&value)
}

fn parse_hcl_value_to_config(value: &hcl::Value) -> Result<Config> {
    let mut config = Config {
        agent_address: None,
        cmd: None,
        cmd_args: None,
        pid_file_name: None,
        cert_dir: None,
        daemon_mode: None,
        add_intermediates_to_bundle: None,
        renew_signal: None,
        svid_file_name: None,
        svid_key_file_name: None,
        svid_bundle_file_name: None,
        jwt_svids: None,
        jwt_bundle_file_name: None,
        include_federated_domains: None,
        cert_file_mode: None,
        key_file_mode: None,
        jwt_bundle_file_mode: None,
        jwt_svid_file_mode: None,
        hint: None,
        omit_expired: None,
        health_checks: None,
    };

    if let hcl::Value::Object(attrs) = value {
        for (key, val) in attrs {
            match key.as_str() {
                "agent_address" => {
                    config.agent_address = extract_string(val);
                }
                "cmd" => {
                    config.cmd = extract_string(val);
                }
                "cmd_args" => {
                    config.cmd_args = extract_string(val);
                }
                "pid_file_name" => {
                    config.pid_file_name = extract_string(val);
                }
                "cert_dir" => {
                    config.cert_dir = extract_string(val);
                }
                "daemon_mode" => {
                    config.daemon_mode = extract_bool(val);
                }
                "add_intermediates_to_bundle" => {
                    config.add_intermediates_to_bundle = extract_bool(val);
                }
                "renew_signal" => {
                    config.renew_signal = extract_string(val);
                }
                "svid_file_name" => {
                    config.svid_file_name = extract_string(val);
                }
                "svid_key_file_name" => {
                    config.svid_key_file_name = extract_string(val);
                }
                "svid_bundle_file_name" => {
                    config.svid_bundle_file_name = extract_string(val);
                }
                "jwt_svids" => {
                    config.jwt_svids = extract_jwt_svids(val);
                }
                "jwt_bundle_file_name" => {
                    config.jwt_bundle_file_name = extract_string(val);
                }
                "include_federated_domains" => {
                    config.include_federated_domains = extract_bool(val);
                }
                "cert_file_mode" => {
                    config.cert_file_mode = extract_string(val);
                }
                "key_file_mode" => {
                    config.key_file_mode = extract_string(val);
                }
                "jwt_bundle_file_mode" => {
                    config.jwt_bundle_file_mode = extract_string(val);
                }
                "jwt_svid_file_mode" => {
                    config.jwt_svid_file_mode = extract_string(val);
                }
                "hint" => {
                    config.hint = extract_string(val);
                }
                "omit_expired" => {
                    config.omit_expired = extract_bool(val);
                }
                "health_checks" => {
                    config.health_checks = extract_health_checks(val);
                }
                _ => {
                    // Ignore unknown keys
                }
            }
        }
    }

    Ok(config)
}

fn extract_string(val: &hcl::Value) -> Option<String> {
    if let hcl::Value::String(s) = val {
        return Some(s.clone());
    }
    None
}

fn extract_bool(val: &hcl::Value) -> Option<bool> {
    if let hcl::Value::Bool(b) = val {
        return Some(*b);
    }
    None
}

fn extract_jwt_svids(val: &hcl::Value) -> Option<Vec<JwtSvid>> {
    let arr = match val {
        hcl::Value::Array(arr) => arr,
        _ => return None,
    };

    let jwt_svids: Vec<JwtSvid> = arr.iter().filter_map(parse_jwt_svid).collect();

    if jwt_svids.is_empty() {
        None
    } else {
        Some(jwt_svids)
    }
}

fn parse_jwt_svid(value: &hcl::Value) -> Option<JwtSvid> {
    let obj = match value {
        hcl::Value::Object(obj) => obj,
        _ => return None,
    };

    let mut jwt_audience = None;
    let mut jwt_extra_audiences = None;
    let mut jwt_svid_file_name = None;

    for (key, val) in obj {
        match key.as_str() {
            "jwt_audience" => {
                jwt_audience = extract_string(val);
            }
            "jwt_extra_audiences" => {
                jwt_extra_audiences = extract_string_array(val);
            }
            "jwt_svid_file_name" => {
                jwt_svid_file_name = extract_string(val);
            }
            _ => {}
        }
    }

    match (jwt_audience, jwt_svid_file_name) {
        (Some(jwt_audience), Some(jwt_svid_file_name)) => Some(JwtSvid {
            jwt_audience,
            jwt_extra_audiences,
            jwt_svid_file_name,
        }),
        _ => None,
    }
}

fn extract_string_array(val: &hcl::Value) -> Option<Vec<String>> {
    match val {
        hcl::Value::Array(arr) => {
            let mut strings = Vec::new();
            for item in arr {
                if let Some(s) = extract_string(item) {
                    strings.push(s);
                }
            }
            if strings.is_empty() {
                None
            } else {
                Some(strings)
            }
        }
        _ => None,
    }
}

fn extract_health_checks(val: &hcl::Value) -> Option<HealthChecks> {
    match val {
        hcl::Value::Object(obj) => {
            let mut health_checks = HealthChecks {
                listener_enabled: None,
                bind_port: None,
                liveness_path: None,
                readiness_path: None,
            };

            for (key, val) in obj {
                match key.as_str() {
                    "listener_enabled" => {
                        health_checks.listener_enabled = extract_bool(val);
                    }
                    "bind_port" => {
                        health_checks.bind_port = extract_port(val).ok();
                    }
                    "liveness_path" => {
                        health_checks.liveness_path = extract_string(val);
                    }
                    "readiness_path" => {
                        health_checks.readiness_path = extract_string(val);
                    }
                    _ => {}
                }
            }

            Some(health_checks)
        }
        _ => None,
    }
}

/// extract a port number from the HCL value
/// 
/// If port number is beyond the legal range [0,65535], an error will be returned. 
fn extract_port(val: &hcl::Value) -> anyhow::Result<u16> {
    if let Some(num) = val.as_u64() {
        if num > 65535 {
            return Err(anyhow::anyhow!("port number MUST not be larger than 65535"))
        }

        return Ok(num as u16)
    }

    Err(anyhow::anyhow!("given value is not a number"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_port_valid() {
        // Test valid port numbers
        assert_eq!(extract_port(&hcl::Value::Number(0u64.into())).unwrap(), 0);
        assert_eq!(extract_port(&hcl::Value::Number(1u64.into())).unwrap(), 1);
        assert_eq!(extract_port(&hcl::Value::Number(8080u64.into())).unwrap(), 8080);
        assert_eq!(extract_port(&hcl::Value::Number(65535u64.into())).unwrap(), 65535);
    }

    #[test]
    fn test_extract_port_invalid_too_large() {
        // Test port numbers beyond u16 range
        let result = extract_port(&hcl::Value::Number(65536u64.into()));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("65535"));

        let result = extract_port(&hcl::Value::Number(100000u64.into()));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("65535"));
    }

    #[test]
    fn test_extract_port_invalid_non_number() {
        // Test non-number values
        let result = extract_port(&hcl::Value::String("8080".to_string()));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a number"));

        let result = extract_port(&hcl::Value::Bool(true));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a number"));

        let result = extract_port(&hcl::Value::Array(vec![]));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a number"));

        let result = extract_port(&hcl::Value::Object(Default::default()));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a number"));
    }
}
