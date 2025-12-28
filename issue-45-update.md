## Current Codebase Analysis (as of 2025-12-28)

### Logging Call Inventory
- **Total instances**: 16 println!/eprintln! calls across 5 source files
- **Files affected**:
  - `src/daemon.rs`: 7 instances
  - `src/workload_api.rs`: 2 instances (includes retry logic)
  - `src/svid.rs`: 2 instances ⚠️ (referenced by integration tests)
  - `src/health.rs`: 3 instances
  - `src/main.rs`: 2 instances

### Detailed Breakdown by File

**File: src/daemon.rs (7 instances)**
- Line 14: `println!("Starting spiffe-helper-rust daemon...");` - **INFO level**
- Line 34: `println!("Daemon running. Waiting for SIGTERM to shutdown...");` - **INFO level**
- Line 40: `println!("Received SIGTERM, shutting down gracefully...");` - **INFO level**
- Line 44: `println!("spiffe-helper-rust daemon is alive");` - **DEBUG level** (periodic heartbeat)
- Line 56: `println!("Health check server exited unexpectedly");` - **WARN level**
- Line 76: `println!("Health check server stopped");` - **INFO level**
- Line 80: `println!("Daemon shutdown complete");` - **INFO level**

**File: src/workload_api.rs (2 instances)**
- Line 56: `eprintln!("Successfully fetched X.509 SVID after {attempt} attempts");` - **INFO level** with context
- Line 67-69: `eprintln!("Attempt {attempt} failed (PermissionDenied), retrying in {delay}s...");` - **WARN level** with retry context

**File: src/svid.rs (2 instances)**
- Line 31: `println!("Fetching X.509 certificate from SPIRE agent at {agent_address}...");` - **INFO level** with structured field
- Line 41: `println!("Successfully fetched and wrote X.509 certificate to {cert_dir}");` - **INFO level** with structured field

**File: src/health.rs (3 instances)**
- Line 35: `println!("Starting health check server on {bind_addr}");` - **INFO level** with structured field
- Line 36: `println!("  Liveness path: {liveness_path}");` - **INFO level** with structured field
- Line 37: `println!("  Readiness path: {readiness_path}");` - **INFO level** with structured field

**File: src/main.rs (2 instances)**
- Line 15: `println!("{VERSION}");` - **INFO level** (version output)
- Line 42: `println!("Running spiffe-helper-rust in one-shot mode...");` - **INFO level**
- Line 44: `println!("One-shot mode complete");` - **INFO level**

### Backward Compatibility Constraint

⚠️ **Critical**: Integration tests rely on exact log message matching:
- `scripts/test-oneshot-x509.sh:269` greps for: `"Successfully fetched and wrote X.509 certificate"`
- `scripts/test-daemon-x509.sh:220` greps for: `"Successfully fetched and wrote X.509 certificate"`

**Implementation must either**:
1. Preserve exact message strings in the new logging output, OR
2. Update the test scripts to match new log format

### Structured Fields Identified

The following data should become structured log fields:
- `agent_address` (src/svid.rs:31)
- `cert_dir` (src/svid.rs:41)
- `attempt` and `delay` (src/workload_api.rs:67-68)
- `bind_addr`, `liveness_path`, `readiness_path` (src/health.rs:35-37)

### Dependencies Required

```toml
# Add to Cargo.toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
```

### Recommended Implementation Approach

**Use `tracing` + `tracing-subscriber`**

**Rationale:**
1. **Async-native**: Project uses `tokio` extensively (daemon mode, health server, workload API client)
2. **Span support**: Can track async operations (certificate fetch with retries, health check server lifecycle)
3. **Industry standard**: Most modern Rust async projects use `tracing`
4. **Rich ecosystem**: Integrates with OpenTelemetry, metrics, distributed tracing
5. **Future-proof**: Supports advanced observability patterns

### Implementation Checklist

**Phase 1: Setup (Low Risk)**
- [ ] Add dependencies to `/home/user/spiffe-helper-rust/Cargo.toml`:
  ```toml
  tracing = "0.1"
  tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
  ```
- [ ] Initialize in `/home/user/spiffe-helper-rust/src/main.rs:main()`:
  ```rust
  tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .init();
  ```

**Phase 2: Replace Logging Calls (Medium Risk)**
- [ ] `/home/user/spiffe-helper-rust/src/daemon.rs`: 7 replacements
- [ ] `/home/user/spiffe-helper-rust/src/workload_api.rs`: 2 replacements with spans
- [ ] `/home/user/spiffe-helper-rust/src/svid.rs`: 2 replacements with structured fields ⚠️ **Keep exact wording for tests**
- [ ] `/home/user/spiffe-helper-rust/src/health.rs`: 3 replacements
- [ ] `/home/user/spiffe-helper-rust/src/main.rs`: 2 replacements

**Phase 3: Add Spans for Async Operations (Enhancement)**
- [ ] Wrap `daemon::run()` in a span
- [ ] Wrap `workload_api::fetch_and_write_x509_svid()` in a span (track retries)
- [ ] Wrap health server lifecycle in a span

**Phase 4: Test Updates (Required if messages change)**
- [ ] `/home/user/spiffe-helper-rust/scripts/test-oneshot-x509.sh:269`: Update grep pattern if needed
- [ ] `/home/user/spiffe-helper-rust/scripts/test-daemon-x509.sh:220`: Update grep pattern if needed

**Phase 5: Documentation**
- [ ] Update `/home/user/spiffe-helper-rust/README.md`: Document `RUST_LOG` environment variable usage
- [ ] Add examples of log filtering (e.g., `RUST_LOG=spiffe_helper_rust=debug`)

### Testing Strategy

1. Run existing integration tests after migration: `make smoke-test`
2. Verify `RUST_LOG` filtering works at different levels
3. Ensure async spans track correctly through retry logic

### Estimated Effort

**Medium (2-4 hours)**
- Low complexity per call site
- Risk managed by comprehensive integration test suite
- Main effort in testing and ensuring backward compatibility
