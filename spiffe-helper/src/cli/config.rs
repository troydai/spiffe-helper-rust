use anyhow::{anyhow, Context, Ok, Result};
use serde::{Deserialize, Serialize};
use std::fs;

use crate::cli::health_check::HealthChecks;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtSvid {
    pub jwt_audience: String,
    pub jwt_extra_audiences: Option<Vec<String>>,
    pub jwt_svid_file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

impl Config {
    #[must_use]
    pub fn svid_file_name(&self) -> &str {
        self.svid_file_name.as_deref().unwrap_or("svid.pem")
    }

    #[must_use]
    pub fn svid_key_file_name(&self) -> &str {
        self.svid_key_file_name.as_deref().unwrap_or("svid_key.pem")
    }

    #[must_use]
    pub fn svid_bundle_file_name(&self) -> &str {
        self.svid_bundle_file_name
            .as_deref()
            .unwrap_or("svid_bundle.pem")
    }

    pub fn agent_address(&self) -> Result<&str> {
        self.agent_address
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("agent_address must be configured"))
    }

    pub fn reconcile_daemon_mode(&mut self, cli_daemon_mode: Option<bool>) {
        if let Some(v) = cli_daemon_mode {
            self.daemon_mode = Some(v);
        }
    }

    #[must_use]
    pub fn is_daemon_mode(&self) -> bool {
        self.daemon_mode.unwrap_or(true)
    }

    /// Validates required configuration fields based on the operation mode.
    ///
    /// Both daemon and one-shot modes require `agent_address` and `cert_dir` to be configured
    /// for X.509 certificate fetching.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if validation passes, or an error with a descriptive message.
    pub fn validate(&self) -> Result<()> {
        let mode_name = if self.is_daemon_mode() {
            "daemon"
        } else {
            "one-shot"
        };

        if self.agent_address.is_none() {
            anyhow::bail!(
                "agent_address must be configured for {mode_name} mode.\n\
                 Set it in your config file: agent_address = \"unix:///run/spire/sockets/agent.sock\""
            );
        }

        if self.cert_dir.is_none() {
            anyhow::bail!(
                "cert_dir must be configured for {mode_name} mode.\n\
                 Set it in your config file: cert_dir = \"/path/to/certs\""
            );
        }

        Ok(())
    }
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
        svid_file_name: Some("svid.pem".to_string()),
        svid_key_file_name: Some("svid_key.pem".to_string()),
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
                    config.agent_address = extract_string(val)?;
                }
                "cmd" => {
                    config.cmd = extract_string(val)?;
                }
                "cmd_args" => {
                    config.cmd_args = extract_string(val)?;
                }
                "pid_file_name" => {
                    config.pid_file_name = extract_string(val)?;
                }
                "cert_dir" => {
                    config.cert_dir = extract_string(val)?;
                }
                "daemon_mode" => {
                    config.daemon_mode = extract_bool(val)?;
                }
                "add_intermediates_to_bundle" => {
                    config.add_intermediates_to_bundle = extract_bool(val)?;
                }
                "renew_signal" => {
                    config.renew_signal = extract_string(val)?;
                }
                "svid_file_name" => {
                    if let Some(s) = extract_string(val)? {
                        config.svid_file_name = Some(s);
                    }
                }
                "svid_key_file_name" => {
                    if let Some(s) = extract_string(val)? {
                        config.svid_key_file_name = Some(s);
                    }
                }
                "svid_bundle_file_name" => {
                    config.svid_bundle_file_name = extract_string(val)?;
                }
                "jwt_svids" => {
                    config.jwt_svids = extract_jwt_svids(val)?;
                }
                "jwt_bundle_file_name" => {
                    config.jwt_bundle_file_name = extract_string(val)?;
                }
                "include_federated_domains" => {
                    config.include_federated_domains = extract_bool(val)?;
                }
                "cert_file_mode" => {
                    config.cert_file_mode = extract_string(val)?;
                }
                "key_file_mode" => {
                    config.key_file_mode = extract_string(val)?;
                }
                "jwt_bundle_file_mode" => {
                    config.jwt_bundle_file_mode = extract_string(val)?;
                }
                "jwt_svid_file_mode" => {
                    config.jwt_svid_file_mode = extract_string(val)?;
                }
                "hint" => {
                    config.hint = extract_string(val)?;
                }
                "omit_expired" => {
                    config.omit_expired = extract_bool(val)?;
                }
                "health_checks" => {
                    config.health_checks = extract_health_checks(val)?;
                }
                _ => {
                    // Ignore unknown keys
                }
            }
        }
    }

    Ok(config)
}

fn extract_string(val: &hcl::Value) -> anyhow::Result<Option<String>> {
    if let hcl::Value::String(s) = val {
        Ok(Some(s.clone()))
    } else {
        Err(anyhow!("given value is not a string"))
    }
}

fn extract_bool(val: &hcl::Value) -> anyhow::Result<Option<bool>> {
    if let hcl::Value::Bool(b) = val {
        Ok(Some(*b))
    } else {
        Err(anyhow!("given value is not a boolean"))
    }
}

fn extract_jwt_svids(val: &hcl::Value) -> anyhow::Result<Option<Vec<JwtSvid>>> {
    let hcl::Value::Array(arr) = val else {
        return Err(anyhow!("given value is not an array"));
    };

    let jwt_svids: Vec<JwtSvid> = arr.iter().filter_map(parse_jwt_svid).collect();

    if jwt_svids.is_empty() {
        Ok(None)
    } else {
        Ok(Some(jwt_svids))
    }
}

fn parse_jwt_svid(value: &hcl::Value) -> Option<JwtSvid> {
    let hcl::Value::Object(obj) = value else {
        return None;
    };

    let mut jwt_audience = None;
    let mut jwt_extra_audiences = None;
    let mut jwt_svid_file_name = None;

    for (key, val) in obj {
        match key.as_str() {
            "jwt_audience" => {
                jwt_audience = extract_string(val).ok().flatten();
            }
            "jwt_extra_audiences" => {
                jwt_extra_audiences = extract_string_array(val).ok().flatten();
            }
            "jwt_svid_file_name" => {
                jwt_svid_file_name = extract_string(val).ok().flatten();
            }
            _ => {}
        }
    }

    if let (Some(jwt_audience), Some(jwt_svid_file_name)) = (jwt_audience, jwt_svid_file_name) {
        Some(JwtSvid {
            jwt_audience,
            jwt_extra_audiences,
            jwt_svid_file_name,
        })
    } else {
        None
    }
}

fn extract_string_array(val: &hcl::Value) -> anyhow::Result<Option<Vec<String>>> {
    if let hcl::Value::Array(arr) = val {
        let mut strings = Vec::new();
        for item in arr {
            let result = extract_string(item)?;
            if let Some(s) = result {
                strings.push(s);
            }
        }
        Ok(Some(strings))
    } else {
        Err(anyhow!("given value is not an array"))
    }
}

/// extract the health check configuration
///
/// The default port is 8080.
fn extract_health_checks(val: &hcl::Value) -> anyhow::Result<Option<HealthChecks>> {
    if let Some(map) = val.as_object() {
        let mut retval = HealthChecks {
            listener_enabled: false,
            bind_port: 8080,
            liveness_path: None,
            readiness_path: None,
        };

        if let Some(v) = map.get("listener_enabled") {
            retval.listener_enabled = extract_bool(v)?.unwrap_or(false);
        }

        // short circuit when health check is not enabled
        if !retval.listener_enabled {
            return Ok(Some(retval));
        }

        if let Some(v) = map.get("bind_port") {
            retval.bind_port = extract_port(v)?;
        }

        if let Some(v) = map.get("liveness_path") {
            retval.liveness_path = extract_string(v)?;
        }

        if let Some(v) = map.get("readiness_path") {
            retval.readiness_path = extract_string(v)?;
        }

        return Ok(Some(retval));
    }

    Err(anyhow!("given HCL value is not a block for health check"))
}

/// extract a port number from the HCL value
///
/// If port number is beyond the legal range [0,65535], an error will be returned.
fn extract_port(val: &hcl::Value) -> anyhow::Result<u16> {
    if let Some(num) = val.as_u64() {
        return u16::try_from(num)
            .map_err(|_| anyhow::anyhow!("port number MUST not be larger than 65535"));
    }

    Err(anyhow!("given value is not a number"))
}

/// Parse file mode from string, supporting both octal (0644) and decimal notation
/// Validates that the mode is in the range 0-0777
pub fn parse_file_mode(mode_str: &str) -> Result<u32> {
    let trimmed = mode_str.trim();

    // If it starts with '0', parse as octal
    let mode = if trimmed.starts_with('0') && trimmed.len() > 1 {
        u32::from_str_radix(trimmed, 8)
            .map_err(|e| anyhow!("Invalid octal file mode '{}': {}", mode_str, e))?
    } else {
        trimmed
            .parse::<u32>()
            .map_err(|e| anyhow!("Invalid file mode '{}': {}", mode_str, e))?
    };

    // Validate range (0-0777)
    if mode > 0o777 {
        return Err(anyhow!(
            "File mode '{}' is out of range (must be 0-0777)",
            mode_str
        ));
    }

    Ok(mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod file_mode_tests {
        use super::*;

        #[test]
        fn test_parse_file_mode_octal() {
            let mode = parse_file_mode("0644").unwrap();
            assert_eq!(mode, 0o644);
        }

        #[test]
        fn test_parse_file_mode_octal_with_leading_zero() {
            let mode = parse_file_mode("0600").unwrap();
            assert_eq!(mode, 0o600);
        }

        #[test]
        fn test_parse_file_mode_decimal() {
            let mode = parse_file_mode("420").unwrap();
            assert_eq!(mode, 420);
        }

        #[test]
        fn test_parse_file_mode_zero() {
            let mode = parse_file_mode("0").unwrap();
            assert_eq!(mode, 0);
        }

        #[test]
        fn test_parse_file_mode_max_valid() {
            let mode = parse_file_mode("0777").unwrap();
            assert_eq!(mode, 0o777);
        }

        #[test]
        fn test_parse_file_mode_with_whitespace() {
            let mode = parse_file_mode("  0644  ").unwrap();
            assert_eq!(mode, 0o644);
        }

        #[test]
        fn test_parse_file_mode_invalid_too_large() {
            let result = parse_file_mode("1000");
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("out of range"));
        }

        #[test]
        fn test_parse_file_mode_invalid_octal() {
            let result = parse_file_mode("0899");
            assert!(result.is_err());
        }

        #[test]
        fn test_parse_file_mode_invalid_string() {
            let result = parse_file_mode("invalid");
            assert!(result.is_err());
        }

        #[test]
        fn test_parse_file_mode_empty() {
            let result = parse_file_mode("");
            assert!(result.is_err());
        }
    }

    fn parse_hcl_value(hcl_str: &str) -> hcl::Value {
        hcl::from_str(hcl_str).expect("Failed to parse HCL")
    }

    fn parse_hcl_simple_value(hcl_str: &str) -> hcl::Value {
        // For simple values, wrap in a key-value pair and extract the value
        let wrapped = format!("key = {hcl_str}");
        let value = parse_hcl_value(&wrapped);
        if let hcl::Value::Object(obj) = value {
            obj.get("key").cloned().expect("key not found")
        } else {
            panic!("Expected object")
        }
    }

    #[test]
    fn test_extract_string_valid() {
        // Arrange
        let value = parse_hcl_simple_value(r#""test""#);

        // Act
        let result = extract_string(&value).unwrap();

        // Assert
        assert_eq!(result, Some("test".to_string()));
    }

    #[test]
    fn test_extract_string_empty() {
        // Arrange
        let value = parse_hcl_simple_value(r#""""#);

        // Act
        let result = extract_string(&value).unwrap();

        // Assert
        assert_eq!(result, Some(String::new()));
    }

    #[test]
    fn test_extract_string_invalid_bool() {
        // Arrange
        let value = parse_hcl_simple_value("true");

        // Act
        let result = extract_string(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a string"));
    }

    #[test]
    fn test_extract_string_invalid_number() {
        // Arrange
        let value = parse_hcl_simple_value("42");

        // Act
        let result = extract_string(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a string"));
    }

    #[test]
    fn test_extract_string_invalid_array() {
        // Arrange
        let value = parse_hcl_simple_value("[]");

        // Act
        let result = extract_string(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a string"));
    }

    #[test]
    fn test_extract_string_invalid_object() {
        // Arrange
        let value = parse_hcl_simple_value("{}");

        // Act
        let result = extract_string(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a string"));
    }

    #[test]
    fn test_extract_bool_true() {
        // Arrange
        let value = parse_hcl_simple_value("true");

        // Act
        let result = extract_bool(&value).unwrap();

        // Assert
        assert_eq!(result, Some(true));
    }

    #[test]
    fn test_extract_bool_false() {
        // Arrange
        let value = parse_hcl_simple_value("false");

        // Act
        let result = extract_bool(&value).unwrap();

        // Assert
        assert_eq!(result, Some(false));
    }

    #[test]
    fn test_extract_bool_invalid_string() {
        // Arrange
        let value = parse_hcl_simple_value(r#""true""#);

        // Act
        let result = extract_bool(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a boolean"));
    }

    #[test]
    fn test_extract_bool_invalid_number() {
        // Arrange
        let value = parse_hcl_simple_value("1");

        // Act
        let result = extract_bool(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a boolean"));
    }

    #[test]
    fn test_extract_bool_invalid_array() {
        // Arrange
        let value = parse_hcl_simple_value("[]");

        // Act
        let result = extract_bool(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a boolean"));
    }

    #[test]
    fn test_extract_bool_invalid_object() {
        // Arrange
        let value = parse_hcl_simple_value("{}");

        // Act
        let result = extract_bool(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a boolean"));
    }

    #[test]
    fn test_extract_string_array_valid() {
        // Arrange
        let value = parse_hcl_simple_value(r#"["a", "b", "c"]"#);

        // Act
        let result = extract_string_array(&value).unwrap();

        // Assert
        assert_eq!(
            result,
            Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        );
    }

    #[test]
    fn test_extract_string_array_empty() {
        // Arrange
        let value = parse_hcl_simple_value("[]");

        // Act
        let result = extract_string_array(&value).unwrap();

        // Assert
        assert_eq!(result, Some(vec![]));
    }

    #[test]
    fn test_extract_string_array_mixed_types() {
        // Arrange
        let value = parse_hcl_simple_value(r#"["a", true, "b"]"#);

        // Act
        let result = extract_string_array(&value);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_string_array_only_non_strings() {
        // Arrange
        let value = parse_hcl_simple_value("[true, 42]");

        // Act
        let result = extract_string_array(&value);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_string_array_invalid_string() {
        // Arrange
        let value = parse_hcl_simple_value(r#""test""#);

        // Act
        let result = extract_string_array(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not an array"));
    }

    #[test]
    fn test_extract_string_array_invalid_bool() {
        // Arrange
        let value = parse_hcl_simple_value("true");

        // Act
        let result = extract_string_array(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not an array"));
    }

    #[test]
    fn test_extract_string_array_invalid_object() {
        // Arrange
        let value = parse_hcl_simple_value("{}");

        // Act
        let result = extract_string_array(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not an array"));
    }

    #[test]
    fn test_parse_jwt_svid_valid() {
        // Arrange
        let hcl_str = r#"
            jwt_audience = "audience1"
            jwt_svid_file_name = "svid1.jwt"
        "#;
        let value = parse_hcl_value(hcl_str);
        let hcl::Value::Object(obj) = value else {
            panic!("Expected object");
        };

        // Act
        let jwt_svid = parse_jwt_svid(&hcl::Value::Object(obj));

        // Assert
        assert!(jwt_svid.is_some());
        let jwt_svid = jwt_svid.unwrap();
        assert_eq!(jwt_svid.jwt_audience, "audience1");
        assert_eq!(jwt_svid.jwt_svid_file_name, "svid1.jwt");
        assert_eq!(jwt_svid.jwt_extra_audiences, None);
    }

    #[test]
    fn test_parse_jwt_svid_with_extra_audiences() {
        // Arrange
        let hcl_str = r#"
            jwt_audience = "audience2"
            jwt_svid_file_name = "svid2.jwt"
            jwt_extra_audiences = ["extra1", "extra2"]
        "#;
        let value = parse_hcl_value(hcl_str);
        let hcl::Value::Object(obj) = value else {
            panic!("Expected object");
        };

        // Act
        let jwt_svid = parse_jwt_svid(&hcl::Value::Object(obj));

        // Assert
        assert!(jwt_svid.is_some());
        let jwt_svid = jwt_svid.unwrap();
        assert_eq!(jwt_svid.jwt_audience, "audience2");
        assert_eq!(jwt_svid.jwt_svid_file_name, "svid2.jwt");
        assert_eq!(
            jwt_svid.jwt_extra_audiences,
            Some(vec!["extra1".to_string(), "extra2".to_string()])
        );
    }

    #[test]
    fn test_parse_jwt_svid_missing_file_name() {
        // Arrange
        let hcl_str = r#"
            jwt_audience = "audience3"
        "#;
        let value = parse_hcl_value(hcl_str);
        let hcl::Value::Object(obj) = value else {
            panic!("Expected object");
        };

        // Act
        let jwt_svid = parse_jwt_svid(&hcl::Value::Object(obj));

        // Assert
        assert!(jwt_svid.is_none());
    }

    #[test]
    fn test_parse_jwt_svid_missing_audience() {
        // Arrange
        let hcl_str = r#"
            jwt_svid_file_name = "svid3.jwt"
        "#;
        let value = parse_hcl_value(hcl_str);
        let hcl::Value::Object(obj) = value else {
            panic!("Expected object");
        };

        // Act
        let jwt_svid = parse_jwt_svid(&hcl::Value::Object(obj));

        // Assert
        assert!(jwt_svid.is_none());
    }

    #[test]
    fn test_parse_jwt_svid_invalid_string() {
        // Arrange
        let value = parse_hcl_simple_value(r#""test""#);

        // Act
        let jwt_svid = parse_jwt_svid(&value);

        // Assert
        assert!(jwt_svid.is_none());
    }

    #[test]
    fn test_parse_jwt_svid_invalid_array() {
        // Arrange
        let value = parse_hcl_simple_value("[]");

        // Act
        let jwt_svid = parse_jwt_svid(&value);

        // Assert
        assert!(jwt_svid.is_none());
    }

    #[test]
    fn test_parse_jwt_svid_invalid_bool() {
        // Arrange
        let value = parse_hcl_simple_value("true");

        // Act
        let jwt_svid = parse_jwt_svid(&value);

        // Assert
        assert!(jwt_svid.is_none());
    }

    #[test]
    fn test_extract_jwt_svids_valid() {
        // Arrange
        let hcl_str = r#"
            jwt_svids = [
                {
                    jwt_audience = "audience1"
                    jwt_svid_file_name = "svid1.jwt"
                },
                {
                    jwt_audience = "audience2"
                    jwt_svid_file_name = "svid2.jwt"
                }
            ]
        "#;
        let value = parse_hcl_value(hcl_str);
        let jwt_svids_val = if let hcl::Value::Object(obj) = &value {
            obj.get("jwt_svids").expect("jwt_svids not found")
        } else {
            panic!("Expected object");
        };

        // Act
        let jwt_svids = extract_jwt_svids(jwt_svids_val).unwrap();

        // Assert
        assert!(jwt_svids.is_some());
        let jwt_svids = jwt_svids.unwrap();
        assert_eq!(jwt_svids.len(), 2);
        assert_eq!(jwt_svids[0].jwt_audience, "audience1");
        assert_eq!(jwt_svids[1].jwt_audience, "audience2");
    }

    #[test]
    fn test_extract_jwt_svids_empty() {
        // Arrange
        let value = parse_hcl_simple_value("[]");

        // Act
        let result = extract_jwt_svids(&value).unwrap();

        // Assert
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_jwt_svids_invalid_entries() {
        // Arrange
        let hcl_str = r#"
            jwt_svids = [
                {
                    jwt_audience = "audience3"
                },
                "invalid"
            ]
        "#;
        let value = parse_hcl_value(hcl_str);
        let jwt_svids_val = if let hcl::Value::Object(obj) = &value {
            obj.get("jwt_svids").expect("jwt_svids not found")
        } else {
            panic!("Expected object");
        };

        // Act
        let result = extract_jwt_svids(jwt_svids_val).unwrap();

        // Assert
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_jwt_svids_invalid_string() {
        // Arrange
        let value = parse_hcl_simple_value(r#""test""#);

        // Act
        let result = extract_jwt_svids(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not an array"));
    }

    #[test]
    fn test_extract_jwt_svids_invalid_bool() {
        // Arrange
        let value = parse_hcl_simple_value("true");

        // Act
        let result = extract_jwt_svids(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not an array"));
    }

    #[test]
    fn test_extract_jwt_svids_invalid_object() {
        // Arrange
        let value = parse_hcl_simple_value("{}");

        // Act
        let result = extract_jwt_svids(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not an array"));
    }

    #[test]
    fn test_extract_health_checks_disabled() {
        // Arrange
        let hcl_str = r"
            listener_enabled = false
        ";
        let value = parse_hcl_value(hcl_str);

        // Act
        let result = extract_health_checks(&value);

        // Assert
        assert!(result.is_ok());
        let health_checks = result.unwrap().unwrap();
        assert!(!health_checks.listener_enabled);
        assert_eq!(health_checks.bind_port, 8080); // default
        assert_eq!(health_checks.liveness_path, None);
        assert_eq!(health_checks.readiness_path, None);
    }

    #[test]
    fn test_extract_health_checks_enabled() {
        // Arrange
        let hcl_str = r#"
            listener_enabled = true
            bind_port = 9090
            liveness_path = "/health/live"
            readiness_path = "/health/ready"
        "#;
        let value = parse_hcl_value(hcl_str);

        // Act
        let result = extract_health_checks(&value);

        // Assert
        assert!(result.is_ok());
        let health_checks = result.unwrap().unwrap();
        assert!(health_checks.listener_enabled);
        assert_eq!(health_checks.bind_port, 9090);
        assert_eq!(
            health_checks.liveness_path,
            Some("/health/live".to_string())
        );
        assert_eq!(
            health_checks.readiness_path,
            Some("/health/ready".to_string())
        );
    }

    #[test]
    fn test_extract_health_checks_partial() {
        // Arrange
        let hcl_str = r"
            listener_enabled = true
            bind_port = 3000
        ";
        let value = parse_hcl_value(hcl_str);

        // Act
        let result = extract_health_checks(&value);

        // Assert
        assert!(result.is_ok());
        let health_checks = result.unwrap().unwrap();
        assert!(health_checks.listener_enabled);
        assert_eq!(health_checks.bind_port, 3000);
        assert_eq!(health_checks.liveness_path, None);
        assert_eq!(health_checks.readiness_path, None);
    }

    #[test]
    fn test_extract_health_checks_defaults() {
        // Arrange
        let hcl_str = r"
        ";
        let value = parse_hcl_value(hcl_str);

        // Act
        let result = extract_health_checks(&value);

        // Assert
        assert!(result.is_ok());
        let health_checks = result.unwrap().unwrap();
        assert!(!health_checks.listener_enabled);
        assert_eq!(health_checks.bind_port, 8080);
        assert_eq!(health_checks.liveness_path, None);
        assert_eq!(health_checks.readiness_path, None);
    }

    #[test]
    fn test_extract_health_checks_invalid_port() {
        // Arrange
        let hcl_str = r"
            listener_enabled = true
            bind_port = 65536
        ";
        let value = parse_hcl_value(hcl_str);

        // Act
        let result = extract_health_checks(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("65535"));
    }

    #[test]
    fn test_extract_health_checks_invalid_string() {
        // Arrange
        let value = parse_hcl_simple_value(r#""test""#);

        // Act
        let result = extract_health_checks(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a block"));
    }

    #[test]
    fn test_extract_health_checks_invalid_array() {
        // Arrange
        let value = parse_hcl_simple_value("[]");

        // Act
        let result = extract_health_checks(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a block"));
    }

    #[test]
    fn test_parse_hcl_value_to_config() {
        // Arrange
        let hcl_str = r#"
            agent_address = "unix:///tmp/agent.sock"
            cmd = "/usr/bin/myapp"
            cmd_args = "--flag value"
            daemon_mode = true
            cert_dir = "/etc/certs"
        "#;
        let value = parse_hcl_value(hcl_str);

        // Act
        let result = parse_hcl_value_to_config(&value);

        // Assert
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(
            config.agent_address,
            Some("unix:///tmp/agent.sock".to_string())
        );
        assert_eq!(config.cmd, Some("/usr/bin/myapp".to_string()));
        assert_eq!(config.cmd_args, Some("--flag value".to_string()));
        assert_eq!(config.daemon_mode, Some(true));
        assert_eq!(config.cert_dir, Some("/etc/certs".to_string()));
    }

    #[test]
    fn test_parse_hcl_value_to_config_empty() {
        // Arrange
        let hcl_str = r"
        ";
        let value = parse_hcl_value(hcl_str);

        // Act
        let result = parse_hcl_value_to_config(&value);

        // Assert
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.agent_address, None);
        assert_eq!(config.cmd, None);
        assert_eq!(config.daemon_mode, None);
        // Defaults
        assert_eq!(config.svid_file_name, Some("svid.pem".to_string()));
        assert_eq!(config.svid_key_file_name, Some("svid_key.pem".to_string()));
    }

    #[test]
    fn test_parse_hcl_value_to_config_unknown_keys() {
        // Arrange
        let hcl_str = r#"
            unknown_key = "value"
            agent_address = "unix:///tmp/agent.sock"
        "#;
        let value = parse_hcl_value(hcl_str);

        // Act
        let result = parse_hcl_value_to_config(&value);

        // Assert
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(
            config.agent_address,
            Some("unix:///tmp/agent.sock".to_string())
        );
    }

    #[test]
    fn test_extract_port_zero() {
        // Arrange
        let value = parse_hcl_simple_value("0");

        // Act
        let result = extract_port(&value).unwrap();

        // Assert
        assert_eq!(result, 0);
    }

    #[test]
    fn test_extract_port_one() {
        // Arrange
        let value = parse_hcl_simple_value("1");

        // Act
        let result = extract_port(&value).unwrap();

        // Assert
        assert_eq!(result, 1);
    }

    #[test]
    fn test_extract_port_common() {
        // Arrange
        let value = parse_hcl_simple_value("8080");

        // Act
        let result = extract_port(&value).unwrap();

        // Assert
        assert_eq!(result, 8080);
    }

    #[test]
    fn test_extract_port_max() {
        // Arrange
        let value = parse_hcl_simple_value("65535");

        // Act
        let result = extract_port(&value).unwrap();

        // Assert
        assert_eq!(result, 65535);
    }

    #[test]
    fn test_extract_port_invalid_too_large_boundary() {
        // Arrange
        let value = parse_hcl_simple_value("65536");

        // Act
        let result = extract_port(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("65535"));
    }

    #[test]
    fn test_extract_port_invalid_too_large_extreme() {
        // Arrange
        let value = parse_hcl_simple_value("100000");

        // Act
        let result = extract_port(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("65535"));
    }

    #[test]
    fn test_extract_port_invalid_string() {
        // Arrange
        let value = parse_hcl_simple_value(r#""8080""#);

        // Act
        let result = extract_port(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a number"));
    }

    #[test]
    fn test_extract_port_invalid_bool() {
        // Arrange
        let value = parse_hcl_simple_value("true");

        // Act
        let result = extract_port(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a number"));
    }

    #[test]
    fn test_extract_port_invalid_array() {
        // Arrange
        let value = parse_hcl_simple_value("[]");

        // Act
        let result = extract_port(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a number"));
    }

    #[test]
    fn test_extract_port_invalid_object() {
        // Arrange
        let value = parse_hcl_simple_value("{}");

        // Act
        let result = extract_port(&value);

        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a number"));
    }

    #[test]
    fn test_config_svid_file_name_accessors() {
        let mut config = Config::default();

        // Test defaults (since Default trait doesn't set defaults in fields automatically, we check behavior with None)
        assert_eq!(config.svid_file_name(), "svid.pem");
        assert_eq!(config.svid_key_file_name(), "svid_key.pem");

        // Test with explicit values
        config.svid_file_name = Some("custom.pem".to_string());
        config.svid_key_file_name = Some("custom_key.pem".to_string());

        assert_eq!(config.svid_file_name(), "custom.pem");
        assert_eq!(config.svid_key_file_name(), "custom_key.pem");
    }

    #[test]
    fn test_is_daemon_mode_defaults_to_true() {
        let config = Config::default();
        assert!(config.is_daemon_mode());
    }

    #[test]
    fn test_is_daemon_mode_respects_setting() {
        let config_false = Config {
            daemon_mode: Some(false),
            ..Default::default()
        };
        assert!(!config_false.is_daemon_mode());

        let config_true = Config {
            daemon_mode: Some(true),
            ..Default::default()
        };
        assert!(config_true.is_daemon_mode());
    }

    #[test]
    fn test_reconcile_daemon_mode() {
        let mut config = Config::default();

        // Initially None, defaults to true
        assert!(config.is_daemon_mode());

        // Override with false
        config.reconcile_daemon_mode(Some(false));
        assert!(!config.is_daemon_mode());

        // No override, keep current value
        config.reconcile_daemon_mode(None);
        assert!(!config.is_daemon_mode());

        // Override with true
        config.reconcile_daemon_mode(Some(true));
        assert!(config.is_daemon_mode());
    }

    #[test]
    fn test_validate_config_missing_agent_address_daemon_mode() {
        let config = Config {
            agent_address: None,
            cert_dir: Some("/tmp/certs".to_string()),
            daemon_mode: Some(true),
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("agent_address must be configured"));
        assert!(error_msg.contains("daemon mode"));
    }

    #[test]
    fn test_validate_config_missing_agent_address_oneshot_mode() {
        let config = Config {
            agent_address: None,
            cert_dir: Some("/tmp/certs".to_string()),
            daemon_mode: Some(false),
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("agent_address must be configured"));
        assert!(error_msg.contains("one-shot mode"));
    }

    #[test]
    fn test_validate_config_missing_cert_dir_daemon_mode() {
        let config = Config {
            agent_address: Some("unix:///tmp/agent.sock".to_string()),
            cert_dir: None,
            daemon_mode: Some(true),
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("cert_dir must be configured"));
        assert!(error_msg.contains("daemon mode"));
    }

    #[test]
    fn test_validate_config_missing_cert_dir_oneshot_mode() {
        let config = Config {
            agent_address: Some("unix:///tmp/agent.sock".to_string()),
            cert_dir: None,
            daemon_mode: Some(false),
            ..Default::default()
        };

        let result = config.validate();
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("cert_dir must be configured"));
        assert!(error_msg.contains("one-shot mode"));
    }

    #[test]
    fn test_validate_config_valid_config() {
        let mut config = Config {
            agent_address: Some("unix:///tmp/agent.sock".to_string()),
            cert_dir: Some("/tmp/certs".to_string()),
            ..Default::default()
        };

        // Should pass for both modes
        config.daemon_mode = Some(true);
        assert!(config.validate().is_ok());

        config.daemon_mode = Some(false);
        assert!(config.validate().is_ok());
    }
}
