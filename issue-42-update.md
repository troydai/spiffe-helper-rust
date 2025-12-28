## Technical Details

### Current Implementation

The validation logic is located in `/home/user/spiffe-helper-rust/src/svid.rs:22-29`:

```rust
let agent_address = config
    .agent_address
    .as_ref()
    .ok_or_else(|| anyhow::anyhow!("agent_address must be configured"))?;
let cert_dir = config
    .cert_dir
    .as_ref()
    .ok_or_else(|| anyhow::anyhow!("cert_dir must be configured"))?;
```

### Scope Issue

Both `agent_address` and `cert_dir` are required for **both daemon mode AND one-shot mode**, not just daemon mode. The current documentation at `README.md:147-148` only mentions "required for daemon mode."

**Code Evidence:**
- Daemon mode path: `main.rs:35` → `daemon.rs:17` → `svid::fetch_x509_certificate()`
- One-shot mode path: `main.rs:43` → `run_once()` → `svid::fetch_x509_certificate()`

### Documentation Gaps

1. **README.md:147-148** - Should say "required" instead of "required for daemon mode" (or add note that it's also required for one-shot mode)

2. **README.md:27-35** (One-Shot Mode section) - Should list required configuration fields:
   - `agent_address`
   - `cert_dir`

3. **Error behavior** - Should explicitly document:
   - Missing `agent_address` causes error: `"agent_address must be configured"`
   - Missing `cert_dir` causes error: `"cert_dir must be configured"`
   - Program exits with code 1 when these validations fail

### Proposed Changes

**Option 1:** Update lines 147-148 to remove "for daemon mode" since these are required for all modes

**Option 2:** Add a note that these fields are required for both daemon and one-shot modes

**Option 3:** Create a "Required Configuration" section that applies to all operation modes

### Recommended Action Items

1. Update documentation at README.md:147-148 to clarify that `agent_address` and `cert_dir` are required for **all operation modes** (both daemon and one-shot)

2. Add configuration requirements to the One-Shot Mode section (README.md:27-35) to explicitly list `agent_address` and `cert_dir` as required

3. Document error messages that users will see when these fields are missing, possibly in a "Troubleshooting" or "Common Errors" section

4. Add example showing the minimal required configuration for both modes
