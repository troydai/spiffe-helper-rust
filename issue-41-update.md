## Additional Context from Codebase Analysis

### Current Implementation Status

The configuration infrastructure is **already in place**:
- Config fields exist: `cert_file_mode` (config.rs:36) and `key_file_mode` (config.rs:37)
- HCL parser extracts these values (config.rs:125-129)
- Values are currently **parsed but unused**

### Security Impact

**Critical**: Private keys are currently written with default permissions using `fs::write()` at:
- workload_api.rs:109 (certificate)
- workload_api.rs:118 (private key)

On Unix systems, this typically results in **0644 permissions**, making private keys readable by:
- File owner (read/write)
- Group members (read)
- Other users (read)

**Expected secure defaults**:
- Private keys: **0600** (owner read/write only)
- Certificates: **0644** (world-readable is acceptable)

### Implementation Checklist

**Code Changes Required**:
1. [ ] Modify `workload_api::fetch_and_write_x509_svid()` signature to accept mode parameters (workload_api.rs:19)
2. [ ] Update call site in `svid::fetch_x509_certificate()` to pass mode values (svid.rs:33-38)
3. [ ] Implement mode string parser (e.g., "0600" → 0o600)
4. [ ] Add permission-setting logic after `fs::write()` calls (workload_api.rs:109-119)
5. [ ] Use `std::fs::set_permissions()` with `std::os::unix::fs::PermissionsExt`
6. [ ] Add `#[cfg(unix)]` guards for Unix-specific code
7. [ ] Add comprehensive tests to `tests/workload_api_tests.rs`

**Default Behavior**:
- If `cert_file_mode` not specified: use **0644**
- If `key_file_mode` not specified: use **0600**

**Platform Support**:
- Unix/Linux: Full support
- Windows: Gracefully ignore (file modes don't apply)

### Related Files
- `/home/user/spiffe-helper-rust/src/config.rs:36-37` - Config struct fields
- `/home/user/spiffe-helper-rust/src/workload_api.rs:109-119` - File writing
- `/home/user/spiffe-helper-rust/src/svid.rs:33-38` - Call site
- `/home/user/spiffe-helper-rust/tests/workload_api_tests.rs` - Tests
- `/home/user/spiffe-helper-rust/IMPLEMENTATION_PLAN.md:145-147` - Design spec

### Example Config Usage

```hcl
agent_address = "unix:///run/spire/sockets/workload_api.sock"
cert_dir = "/etc/certs"
cert_file_mode = "0644"  # Certificate readable by all
key_file_mode = "0600"   # Private key owner-only
```
