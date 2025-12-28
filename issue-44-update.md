## Code References

**No renewal logic exists:**
- `/home/user/spiffe-helper-rust/src/daemon.rs:17` - Single `fetch_x509_certificate()` call at startup
- `/home/user/spiffe-helper-rust/src/daemon.rs:37-70` - Main loop has no certificate update handling
- `/home/user/spiffe-helper-rust/src/workload_api.rs:52` - Uses one-shot `fetch_x509_svid()`, not streaming

**Configuration support already exists but is unused:**
- `/home/user/spiffe-helper-rust/src/config.rs:29` - `renew_signal` field defined
- `/home/user/spiffe-helper-rust/src/config.rs:104-106` - `renew_signal` parsed from HCL
- No code references this field anywhere

## Implementation Requirements

### Phase 1: Minimum Viable Implementation
1. Add periodic certificate checking to daemon loop
2. Calculate next refresh time as `(NotAfter - NotBefore) / 2`
3. Fetch new certificate before expiry
4. Write atomically using temp file + rename pattern

### Phase 2: Enhanced Implementation
1. Implement streaming watch using spiffe crate's watch API
2. Handle updates asynchronously in daemon loop
3. Add renew_signal support (send SIGHUP/SIGUSR1 to process)

### Technical Decisions Needed

**Q1:** Periodic polling vs. streaming watch?
- **Polling**: Simpler, works with current `fetch_x509_svid()` API
- **Streaming**: More efficient, requires exploring spiffe crate's watch capabilities

**Q2:** When to schedule first renewal?
- Industry standard: 50% of certificate lifetime
- Go implementation: Uses `getRefreshInterval()` function

**Q3:** Error handling during renewal?
- Retry with exponential backoff (already exists for initial fetch)
- Keep existing certificate if renewal fails?
- Health check should reflect renewal failures

### Files Requiring Changes

- `src/daemon.rs` - Add renewal logic to main loop
- `src/workload_api.rs` - Add streaming/watch or periodic fetch
- `src/svid.rs` - May need atomic file writing helper
- `src/config.rs` - Already supports renew_signal
- `src/health.rs` - Track renewal status for health checks

### Testing Strategy

- Unit test: Certificate expiry calculation
- Integration test: Simulate certificate rotation with mock SPIRE
- End-to-end: Deploy to kind cluster and verify rotation (infrastructure exists in `Makefile`)

### Critical Impact

1. **Certificates will expire** - X.509 SVIDs typically have short lifespans (1 hour to 24 hours). The daemon will hold expired certificates until manually restarted.
2. **Production outages** - Services relying on these certificates will fail when they expire.
3. **No automatic recovery** - Without renewal, manual intervention is required.

### Priority Level

**P0 (Essential for production use)** - As documented in IMPLEMENTATION_PLAN.md:27, this is the highest priority missing feature preventing production deployment of spiffe-helper-rust.
