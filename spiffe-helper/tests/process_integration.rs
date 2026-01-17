//! Integration tests for process spawning with `cmd_args` parsing.
//!
//! These tests verify that the `cmd_args` parsing integrates correctly with
//! process spawning, testing the full flow from configuration to execution.

use spiffe_helper::process::parse_cmd_args;
use std::process::Command;
use tempfile::tempdir;

/// Test that `parse_cmd_args` correctly parses arguments that can be used with Command.
#[test]
fn test_parse_cmd_args_with_command_execution() {
    // Parse arguments for echo command
    let args = parse_cmd_args("hello world").unwrap();
    assert_eq!(args, vec!["hello", "world"]);

    // Execute the command with parsed arguments
    let output = Command::new("echo").args(&args).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "hello world");
}

/// Test parsing and executing a command with quoted arguments containing spaces.
#[test]
fn test_parse_cmd_args_quoted_with_spaces() {
    // Parse arguments with quoted string containing spaces
    let args = parse_cmd_args(r#""hello world""#).unwrap();
    assert_eq!(args, vec!["hello world"]);

    // Execute echo with the parsed argument
    let output = Command::new("echo").args(&args).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "hello world");
}

/// Test that shell-style command strings work correctly with /bin/sh.
#[test]
fn test_parse_cmd_args_shell_command() {
    // This simulates how spiffe-helper uses cmd_args with a shell
    let args = parse_cmd_args(r#"-c "echo test output""#).unwrap();
    assert_eq!(args, vec!["-c", "echo test output"]);

    // Execute via shell
    let output = Command::new("/bin/sh").args(&args).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "test output");
}

/// Test complex shell command with multiple arguments and special characters.
#[test]
fn test_parse_cmd_args_complex_shell_command() {
    // Complex command similar to what might be used in real configurations
    let args = parse_cmd_args(r#"-c "echo 'first'; echo 'second'""#).unwrap();
    assert_eq!(args, vec!["-c", "echo 'first'; echo 'second'"]);

    let output = Command::new("/bin/sh").args(&args).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines, vec!["first", "second"]);
}

/// Test that environment variables in quoted strings are preserved (not expanded by parser).
#[test]
fn test_parse_cmd_args_preserves_env_vars() {
    // The parser should not expand environment variables
    let args = parse_cmd_args(r#"-c "echo $HOME""#).unwrap();
    assert_eq!(args, vec!["-c", "echo $HOME"]);

    // When executed by shell, $HOME should be expanded
    let output = Command::new("/bin/sh").args(&args).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The output should be a path, not literally "$HOME"
    assert!(!stdout.trim().is_empty());
    assert!(!stdout.contains("$HOME"));
}

/// Test file path with spaces in arguments.
#[test]
fn test_parse_cmd_args_file_path_with_spaces() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("file with spaces.txt");
    std::fs::write(&file_path, "content").unwrap();

    // Create args string with quoted path
    let args_str = format!(r#"-c "cat '{}'""#, file_path.display());
    let args = parse_cmd_args(&args_str).unwrap();

    let output = Command::new("/bin/sh").args(&args).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "content");
}

/// Test multiple flags and arguments.
#[test]
fn test_parse_cmd_args_multiple_flags() {
    let args = parse_cmd_args("-a -b -c value --long-opt=test").unwrap();
    assert_eq!(args, vec!["-a", "-b", "-c", "value", "--long-opt=test"]);
}

/// Test empty string returns empty vector.
#[test]
fn test_parse_cmd_args_empty_string() {
    let args = parse_cmd_args("").unwrap();
    assert!(args.is_empty());
}

/// Test whitespace-only string returns empty vector.
#[test]
fn test_parse_cmd_args_whitespace_only() {
    let args = parse_cmd_args("   \t  \n  ").unwrap();
    assert!(args.is_empty());
}

/// Test unclosed quote returns error.
#[test]
fn test_parse_cmd_args_unclosed_quote_error() {
    let result = parse_cmd_args(r#"-c "unclosed"#);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Failed to parse cmd_args"));
}

/// Test nginx-style configuration arguments.
#[test]
fn test_parse_cmd_args_nginx_style() {
    // Real-world example: nginx configuration
    let args = parse_cmd_args(r"-c /etc/nginx/nginx.conf -g 'daemon off;'").unwrap();
    assert_eq!(
        args,
        vec!["-c", "/etc/nginx/nginx.conf", "-g", "daemon off;"]
    );
}

/// Test that backslash escapes work correctly.
#[test]
fn test_parse_cmd_args_backslash_escapes() {
    // Escaped space
    let args = parse_cmd_args(r"file\ name").unwrap();
    assert_eq!(args, vec!["file name"]);

    // Escaped quote inside quoted string
    let args = parse_cmd_args(r#""file\"name""#).unwrap();
    assert_eq!(args, vec!["file\"name"]);
}

/// Test mixed single and double quotes.
#[test]
fn test_parse_cmd_args_mixed_quotes() {
    let args = parse_cmd_args(r#"'single quoted' "double quoted""#).unwrap();
    assert_eq!(args, vec!["single quoted", "double quoted"]);
}

/// Test that parsed arguments work with tokio Command (async).
#[tokio::test]
async fn test_parse_cmd_args_with_tokio_command() {
    use tokio::process::Command as TokioCommand;

    let args = parse_cmd_args(r#"-c "echo async test""#).unwrap();

    let output = TokioCommand::new("/bin/sh")
        .args(&args)
        .output()
        .await
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "async test");
}

/// Test spawning a process that runs briefly and exits.
#[tokio::test]
async fn test_spawn_process_with_parsed_args() {
    use tokio::process::Command as TokioCommand;

    let args = parse_cmd_args(r#"-c "exit 0""#).unwrap();

    let mut child = TokioCommand::new("/bin/sh").args(&args).spawn().unwrap();

    let status = child.wait().await.unwrap();
    assert!(status.success());
}

/// Test spawning a process with non-zero exit code.
#[tokio::test]
async fn test_spawn_process_with_exit_code() {
    use tokio::process::Command as TokioCommand;

    let args = parse_cmd_args(r#"-c "exit 42""#).unwrap();

    let mut child = TokioCommand::new("/bin/sh").args(&args).spawn().unwrap();

    let status = child.wait().await.unwrap();
    assert!(!status.success());
    assert_eq!(status.code(), Some(42));
}

/// Test that a spawned process can be killed.
#[tokio::test]
async fn test_spawn_and_kill_process() {
    use tokio::process::Command as TokioCommand;
    use tokio::time::{timeout, Duration};

    // Spawn a long-running process
    let args = parse_cmd_args(r#"-c "sleep 60""#).unwrap();

    let mut child = TokioCommand::new("/bin/sh").args(&args).spawn().unwrap();

    // Verify it's running
    let pid = child.id();
    assert!(pid.is_some());

    // Kill it
    child.kill().await.unwrap();

    // Wait for it to exit (with timeout)
    let result = timeout(Duration::from_secs(5), child.wait()).await;
    assert!(result.is_ok());
}

/// Test process spawning with environment variable in args.
#[tokio::test]
async fn test_spawn_process_with_env_expansion() {
    use tokio::process::Command as TokioCommand;

    // Set a custom env var and verify it's expanded by the shell
    let args = parse_cmd_args(r#"-c "echo $TEST_VAR""#).unwrap();

    let output = TokioCommand::new("/bin/sh")
        .args(&args)
        .env("TEST_VAR", "integration_test_value")
        .output()
        .await
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "integration_test_value");
}
