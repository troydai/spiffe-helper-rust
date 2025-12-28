## Update: Current Test Location

**Note**: Tests are located in `/home/user/spiffe-helper-rust/tests/workload_api_tests.rs`, not in `src/workload_api.rs`. The implementation under test is in `src/workload_api.rs:19-122`.

## Current Test Coverage

The test file contains only 3 tests, all covering error/edge cases:

1. **`test_fetch_and_write_x509_svid_invalid_address`** (lines 5-17) - Tests invalid agent address handling
2. **`test_fetch_and_write_x509_svid_missing_agent`** (lines 19-39) - Tests non-existent unix socket
3. **`test_cert_dir_creation`** (lines 41-54) - Tests directory creation logic only (not the full function)

## Implementation Details to Test

The `fetch_and_write_x509_svid` function (src/workload_api.rs:19-122) includes:

### 1. Certificate Chain Handling (lines 97-107)
- Iterates over `svid.cert_chain()`
- PEM-encodes each certificate with "CERTIFICATE" tag
- Joins certificates with newlines
- **Test needed**: Verify chain with 1, 2, and 3+ certificates

### 2. Default Filenames (lines 91-92)
- Certificate: `svid.pem`
- Private key: `svid_key.pem`
- **Test needed**: Verify defaults when parameters are None

### 3. Custom Filenames (lines 91-94)
- Accepts optional `svid_file_name` and `svid_key_file_name`
- **Test needed**: Verify custom names are respected

### 4. Retry Logic (lines 51-81)
- Retries up to 10 times on PermissionDenied
- Exponential backoff: 1s, 2s, 4s, 8s, 16s (max)
- **Test needed**: Mock PermissionDenied and verify retry behavior

### 5. PEM Format
- Certificate tag: "CERTIFICATE"
- Private key tag: "PRIVATE KEY"
- **Test needed**: Verify exact PEM format of output files

### 6. Private Key Writing (lines 113-119)
- Private key encoded with "PRIVATE KEY" tag
- **Test needed**: Verify key file contents and format

## Testing Approach

### Option 1: Mock Workload API Server (Recommended)

Create a test helper that spawns a mock gRPC server implementing the Workload API:
- Use `tonic::transport::Server` (already in dependencies)
- Return test SVID data with controllable certificate chains
- Allow testing both success and error scenarios

### Option 2: Test Fixtures

If mocking the spiffe client is complex:
- Focus on testing the PEM encoding logic separately
- Use pre-generated test certificate data
- May require refactoring to separate concerns

## Dependencies Available

From `Cargo.toml`:
- `tokio` (async runtime) - Line 19
- `tonic` (gRPC server) - Line 25
- `tempfile` (temp directories) - Line 28
- `spiffe` (SPIFFE types) - Line 23
- `pem` (PEM encoding) - Line 24

## Missing Test Scenarios

All items from the original issue are still missing:

1. ❌ **Successful certificate writing** - No tests with mock SPIRE agent
2. ❌ **Custom filenames** - No tests for `svid_file_name` and `svid_key_file_name` parameters
3. ❌ **Certificate chain handling** - No tests for single vs. multiple certificate chains
4. ❌ **File permissions** - Cannot be tested until Issue #41 is implemented
5. ❌ **Retry logic verification** - No tests for PermissionDenied retry behavior
6. ❌ **PEM format validation** - No tests verifying exact output format

## File References

- **Implementation**: `/home/user/spiffe-helper-rust/src/workload_api.rs:19-122`
- **Existing tests**: `/home/user/spiffe-helper-rust/tests/workload_api_tests.rs`
- **Config definitions**: `/home/user/spiffe-helper-rust/src/config.rs:36-37`
- **Dependencies**: `/home/user/spiffe-helper-rust/Cargo.toml`

## Recommended Test Structure

```rust
#[cfg(test)]
mod tests {
    // Test 1: Successful fetch with single certificate
    #[tokio::test]
    async fn test_fetch_single_certificate() { }

    // Test 2: Successful fetch with certificate chain (leaf + intermediates)
    #[tokio::test]
    async fn test_fetch_certificate_chain() { }

    // Test 3: Custom filenames
    #[tokio::test]
    async fn test_custom_filenames() { }

    // Test 4: Default filenames
    #[tokio::test]
    async fn test_default_filenames() { }

    // Test 5: PEM format validation
    #[tokio::test]
    async fn test_pem_format() { }

    // Test 6: Retry logic on PermissionDenied
    #[tokio::test]
    async fn test_retry_on_permission_denied() { }
}
```

## Implementation Challenge

The main challenge is creating a mock Workload API server or finding test utilities in the spiffe crate. Consider investigating the spiffe crate's source code for any existing test helpers before implementing a custom mock server.

## Priority

This issue improves code reliability significantly and is well-defined. The test infrastructure is currently minimal, making this a valuable contribution to the project's quality.
