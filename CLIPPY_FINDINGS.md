# Clippy Lint Findings

**Date**: 2025-12-24
**Clippy Version**: 1.91.0
**Command**: `cargo clippy --all-targets --all-features -- -D warnings`

## Summary

Clippy found **26 violations** of the `clippy::excessive-nesting` lint across 3 source files.

## Detailed Findings

### Configuration: src/config.rs

**Violation**: `clippy::excessive-nesting`
**Count**: 23 instances
**Nesting Threshold**: 3 (configured in clippy.toml)

All violations occur in the `parse_hcl_value_to_config()` function (lines 55-154), specifically in the match statement that parses configuration fields:

```rust
// Lines 83-148: Each match arm exceeds nesting threshold
if let hcl::Value::Object(attrs) = value {
    for (key, val) in attrs {          // Nesting level 1
        match key.as_str() {            // Nesting level 2
            "agent_address" => {        // Nesting level 3
                config.agent_address = extract_string(val)?;  // Level 4 - EXCEEDS THRESHOLD
            }
            // ... 20 more similar cases
        }
    }
}
```

**Affected lines**:
- 83-85: `agent_address`
- 86-88: `cmd`
- 89-91: `cmd_args`
- 92-94: `pid_file_name`
- 95-97: `cert_dir`
- 98-100: `daemon_mode`
- 101-103: `add_intermediates_to_bundle`
- 104-106: `renew_signal`
- 107-109: `svid_file_name`
- 110-112: `svid_key_file_name`
- 113-115: `svid_bundle_file_name`
- 116-118: `jwt_svids`
- 119-121: `jwt_bundle_file_name`
- 122-124: `include_federated_domains`
- 125-127: `cert_file_mode`
- 128-130: `key_file_mode`
- 131-133: `jwt_bundle_file_mode`
- 134-136: `jwt_svid_file_mode`
- 137-139: `hint`
- 140-142: `omit_expired`
- 143-145: `health_checks`
- 146-148: default case (`_`)

Additionally:
- Line 230-232: `extract_string_array()` function has nested if statement

### Workload API: src/workload_api.rs

**Violation**: `clippy::excessive-nesting`
**Count**: 2 instances

1. **Line 55-57**: Success logging after retry
   ```rust
   Ok(s) => {
       svid = Some(s);
       if attempt > 1 {  // Nested if
           eprintln!("Successfully fetched X.509 SVID after {attempt} attempts");
       }
   ```

2. **Lines 64-72**: Retry logic with exponential backoff
   ```rust
   Err(e) => {
       let error_str = format!("{e:?}");
       last_error_msg = Some(format!("{e} ({error_str})"));
       if error_str.contains("PermissionDenied") && attempt < 10 {  // Nested if
           let delay = std::cmp::min(1u64 << (attempt - 1), 16);
           eprintln!("Attempt {attempt} failed (PermissionDenied), retrying in {delay}s...");
           tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
           continue;
       }
   ```

### Main: src/main.rs

**Violation**: `clippy::excessive-nesting`
**Count**: 1 instance

**Line 155-159**: Health check server initialization
```rust
if let Some(ref health_checks) = health_checks {
    if health_checks.listener_enabled {
        // ... setup code ...
        Some(tokio::spawn(async move {  // Nested async block
            axum::serve(listener, app)
                .await
                .context("Health check server failed")
        }))
```

## Recommended Fixes

### For src/config.rs

**Option 1**: Extract match arms into helper function
```rust
fn parse_config_field(key: &str, val: &hcl::Value, config: &mut Config) -> Result<()> {
    match key {
        "agent_address" => config.agent_address = extract_string(val)?,
        "cmd" => config.cmd = extract_string(val)?,
        // ... rest of fields
        _ => {} // Ignore unknown keys
    }
    Ok(())
}

fn parse_hcl_value_to_config(value: &hcl::Value) -> Result<Config> {
    let mut config = Config::default();
    if let hcl::Value::Object(attrs) = value {
        for (key, val) in attrs {
            parse_config_field(key.as_str(), val, &mut config)?;
        }
    }
    Ok(config)
}
```

**Option 2**: Use early continue for cleaner structure
```rust
if let hcl::Value::Object(attrs) = value {
    for (key, val) in attrs {
        let _ = match key.as_str() {
            "agent_address" => config.agent_address = extract_string(val)?,
            "cmd" => config.cmd = extract_string(val)?,
            // Flatten into single expression per arm
            _ => continue,
        };
    }
}
```

### For src/workload_api.rs

Use early continue or extract functions:
```rust
// For retry logging
Ok(s) => {
    svid = Some(s);
    log_retry_success(attempt);  // Extract to function
    break;
}

// For error handling
Err(e) => {
    if should_retry(&e, attempt) {
        perform_retry(attempt).await;
        continue;
    }
    return Err(create_error(e, attempt, &last_error_msg));
}
```

### For src/main.rs

Extract health server initialization:
```rust
async fn start_health_server(health_checks: &HealthChecks) -> Result<JoinHandle<Result<()>>> {
    if !health_checks.listener_enabled {
        return Ok(None);
    }
    // ... setup code ...
    Ok(Some(tokio::spawn(async move {
        axum::serve(listener, app).await.context("Health check server failed")
    })))
}

// In run_daemon:
let health_server_handle = start_health_server(&health_checks).await?;
```

## Impact

- **Severity**: Low (code quality issue, not a bug)
- **Blocks CI**: Yes - clippy is run with `-D warnings`
- **Introduced by**: PR #80 (clippy configuration setup)
- **Resolution Priority**: Medium - should be fixed to unblock development

## Related

- Issue #80: Setup clippy configuration
- Config file: `clippy.toml` (excessive-nesting-threshold = 3)
