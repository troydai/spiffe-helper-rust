//! Integration tests for cmd_args configuration parsing.
//!
//! These tests verify that cmd_args is correctly parsed from HCL configuration
//! files and can be used with the process module.

use spiffe_helper_rust::cli::config::parse_hcl_config;
use spiffe_helper_rust::process::parse_cmd_args;
use std::io::Write;
use tempfile::NamedTempFile;

/// Helper to create a temp config file with given content.
fn create_temp_config(content: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(content.as_bytes()).unwrap();
    file.flush().unwrap();
    file
}

/// Test parsing a config with simple cmd_args.
#[test]
fn test_config_with_simple_cmd_args() {
    let config_content = r#"
        agent_address = "unix:///run/spire/sockets/agent.sock"
        cert_dir = "/tmp/certs"
        cmd = "/usr/bin/nginx"
        cmd_args = "-c /etc/nginx/nginx.conf"
    "#;

    let file = create_temp_config(config_content);
    let config = parse_hcl_config(file.path()).unwrap();

    assert_eq!(config.cmd, Some("/usr/bin/nginx".to_string()));
    assert_eq!(
        config.cmd_args,
        Some("-c /etc/nginx/nginx.conf".to_string())
    );

    // Verify the args can be parsed
    let args = parse_cmd_args(config.cmd_args.as_ref().unwrap()).unwrap();
    assert_eq!(args, vec!["-c", "/etc/nginx/nginx.conf"]);
}

/// Test parsing a config with quoted cmd_args containing spaces.
#[test]
fn test_config_with_quoted_cmd_args() {
    let config_content = r#"
        agent_address = "unix:///run/spire/sockets/agent.sock"
        cert_dir = "/tmp/certs"
        cmd = "/bin/sh"
        cmd_args = "-c \"echo hello world\""
    "#;

    let file = create_temp_config(config_content);
    let config = parse_hcl_config(file.path()).unwrap();

    assert_eq!(config.cmd, Some("/bin/sh".to_string()));
    assert_eq!(
        config.cmd_args,
        Some(r#"-c "echo hello world""#.to_string())
    );

    // Verify the args can be parsed
    let args = parse_cmd_args(config.cmd_args.as_ref().unwrap()).unwrap();
    assert_eq!(args, vec!["-c", "echo hello world"]);
}

/// Test parsing a config with complex cmd_args (nginx daemon off style).
#[test]
fn test_config_with_complex_cmd_args() {
    let config_content = r#"
        agent_address = "unix:///run/spire/sockets/agent.sock"
        cert_dir = "/tmp/certs"
        cmd = "/usr/bin/nginx"
        cmd_args = "-c /etc/nginx/nginx.conf -g 'daemon off;'"
    "#;

    let file = create_temp_config(config_content);
    let config = parse_hcl_config(file.path()).unwrap();

    // Verify the args can be parsed correctly
    let args = parse_cmd_args(config.cmd_args.as_ref().unwrap()).unwrap();
    assert_eq!(
        args,
        vec!["-c", "/etc/nginx/nginx.conf", "-g", "daemon off;"]
    );
}

/// Test parsing a config with path containing spaces in cmd_args.
#[test]
fn test_config_with_path_spaces_in_cmd_args() {
    let config_content = r#"
        agent_address = "unix:///run/spire/sockets/agent.sock"
        cert_dir = "/tmp/certs"
        cmd = "/bin/cat"
        cmd_args = "\"/path/with spaces/config.conf\""
    "#;

    let file = create_temp_config(config_content);
    let config = parse_hcl_config(file.path()).unwrap();

    // Verify the args can be parsed correctly
    let args = parse_cmd_args(config.cmd_args.as_ref().unwrap()).unwrap();
    assert_eq!(args, vec!["/path/with spaces/config.conf"]);
}

/// Test parsing a config without cmd_args.
#[test]
fn test_config_without_cmd_args() {
    let config_content = r#"
        agent_address = "unix:///run/spire/sockets/agent.sock"
        cert_dir = "/tmp/certs"
        cmd = "/usr/bin/nginx"
    "#;

    let file = create_temp_config(config_content);
    let config = parse_hcl_config(file.path()).unwrap();

    assert_eq!(config.cmd, Some("/usr/bin/nginx".to_string()));
    assert_eq!(config.cmd_args, None);
}

/// Test parsing a config without cmd (no managed process).
#[test]
fn test_config_without_cmd() {
    let config_content = r#"
        agent_address = "unix:///run/spire/sockets/agent.sock"
        cert_dir = "/tmp/certs"
        daemon_mode = true
    "#;

    let file = create_temp_config(config_content);
    let config = parse_hcl_config(file.path()).unwrap();

    assert_eq!(config.cmd, None);
    assert_eq!(config.cmd_args, None);
}

/// Test parsing a config with empty cmd_args.
#[test]
fn test_config_with_empty_cmd_args() {
    let config_content = r#"
        agent_address = "unix:///run/spire/sockets/agent.sock"
        cert_dir = "/tmp/certs"
        cmd = "/usr/bin/nginx"
        cmd_args = ""
    "#;

    let file = create_temp_config(config_content);
    let config = parse_hcl_config(file.path()).unwrap();

    assert_eq!(config.cmd_args, Some("".to_string()));

    // Verify empty args can be parsed
    let args = parse_cmd_args(config.cmd_args.as_ref().unwrap()).unwrap();
    assert!(args.is_empty());
}

/// Test parsing a config with cmd_args and renew_signal for managed process.
#[test]
fn test_config_managed_process_with_signal() {
    let config_content = r#"
        agent_address = "unix:///run/spire/sockets/agent.sock"
        cert_dir = "/tmp/certs"
        daemon_mode = true
        cmd = "/bin/sh"
        cmd_args = "-c \"trap 'echo received' USR1; while true; do sleep 1; done\""
        renew_signal = "SIGUSR1"
    "#;

    let file = create_temp_config(config_content);
    let config = parse_hcl_config(file.path()).unwrap();

    assert_eq!(config.cmd, Some("/bin/sh".to_string()));
    assert_eq!(config.renew_signal, Some("SIGUSR1".to_string()));

    // Verify the args can be parsed
    let args = parse_cmd_args(config.cmd_args.as_ref().unwrap()).unwrap();
    assert_eq!(args.len(), 2);
    assert_eq!(args[0], "-c");
    assert!(args[1].contains("trap"));
}

/// Test full config with all managed process options.
#[test]
fn test_full_managed_process_config() {
    let config_content = r#"
        agent_address = "unix:///run/spire/sockets/agent.sock"
        cert_dir = "/tmp/certs"
        daemon_mode = true

        # Managed process configuration
        cmd = "/usr/sbin/nginx"
        cmd_args = "-c /etc/nginx/nginx.conf -g 'daemon off;'"
        renew_signal = "SIGHUP"

        # Health checks
        health_checks {
            listener_enabled = true
            bind_port = 8080
            liveness_path = "/healthz"
            readiness_path = "/ready"
        }
    "#;

    let file = create_temp_config(config_content);
    let config = parse_hcl_config(file.path()).unwrap();

    // Verify all fields
    assert_eq!(
        config.agent_address,
        Some("unix:///run/spire/sockets/agent.sock".to_string())
    );
    assert_eq!(config.cert_dir, Some("/tmp/certs".to_string()));
    assert_eq!(config.daemon_mode, Some(true));
    assert_eq!(config.cmd, Some("/usr/sbin/nginx".to_string()));
    assert_eq!(config.renew_signal, Some("SIGHUP".to_string()));

    // Verify cmd_args parsing
    let args = parse_cmd_args(config.cmd_args.as_ref().unwrap()).unwrap();
    assert_eq!(
        args,
        vec!["-c", "/etc/nginx/nginx.conf", "-g", "daemon off;"]
    );

    // Verify health checks
    let health = config.health_checks.unwrap();
    assert!(health.listener_enabled);
    assert_eq!(health.bind_port, 8080);
}

/// Test that escaped quotes in HCL are handled correctly.
#[test]
fn test_config_escaped_quotes_in_hcl() {
    let config_content = r#"
        agent_address = "unix:///run/spire/sockets/agent.sock"
        cert_dir = "/tmp/certs"
        cmd = "/bin/sh"
        cmd_args = "-c \"echo \\\"quoted\\\"\""
    "#;

    let file = create_temp_config(config_content);
    let config = parse_hcl_config(file.path()).unwrap();

    // The HCL parser should unescape the outer quotes
    // The cmd_args should contain: -c "echo \"quoted\""
    let args = parse_cmd_args(config.cmd_args.as_ref().unwrap()).unwrap();
    assert_eq!(args.len(), 2);
    assert_eq!(args[0], "-c");
    // The shell argument should contain escaped quotes
    assert!(args[1].contains("echo"));
}
