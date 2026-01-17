# spiffe-helper Implementation Plan

This document tracks the progress of reimplementing [spiffe-helper](https://github.com/spiffe/spiffe-helper) in Rust with 100% feature parity.

## Current Implementation Status

### Completed Features

| Feature | Status | Notes |
|---------|--------|-------|
| CLI argument parsing | Done | `--config`, `--daemon-mode`, `--version` |
| HCL configuration parsing | Done | Full parser with type-safe extraction |
| Daemon mode | Done | Continuous running with graceful shutdown |
| One-shot mode | Done | Single fetch and exit |
| X.509 SVID fetching | Done | Fetches from SPIFFE Workload API |
| X.509 certificate writing | Done | Writes cert chain + private key to PEM |
| Retry with exponential backoff | Done | Up to 10 attempts, max 16s backoff |
| SIGTERM signal handling | Done | Graceful shutdown in daemon mode |
| Health check HTTP server | Done | Liveness and readiness endpoints |
| Configurable health paths | Done | Custom paths for probes |
| Custom cert/key filenames | Done | `svid_file_name`, `svid_key_file_name` |

### Features Not Yet Implemented

| Feature | Priority | Go Reference |
|---------|----------|--------------|
| Certificate renewal/rotation | P0 | `updateCertificates()`, X509 watcher |
| X.509 bundle writing | P1 | `SVIDBundleFilename` config |
| Process signaling (renew_signal) | P1 | `signalProcess()`, `signalPIDFile()` |
| Command execution (cmd/cmd_args) | P1 | `signalProcess()` first call |
| JWT SVID fetching | P2 | `fetchAndWriteJWTSVIDs()` |
| JWT bundle fetching | P2 | `fetchAndWriteJWTBundle()` |
| File permission modes | P2 | `cert_file_mode`, `key_file_mode`, etc. |
| PID file support | P2 | `pid_file_name` config |
| Add intermediates to bundle | P3 | `add_intermediates_to_bundle` config |
| Include federated domains | P3 | `include_federated_domains` config |
| Omit expired certificates | P3 | `omit_expired` config |
| SVID hint selection | P3 | `hint` config |
| Dynamic health status | P3 | Track cert write success/failure |
| Windows signal support | P4 | Windows-specific handling |

---

## Gap Analysis

### 1. Certificate Lifecycle Management (Critical Gap)

**Current state:** The Rust implementation fetches X.509 SVIDs once at startup but does not watch for updates or handle certificate rotation.

**Go implementation:**
- Uses `workloadapi.WatchX509Context()` to continuously monitor for certificate updates
- Implements `OnX509ContextUpdate()` callback to handle new certificates
- Calculates refresh intervals based on certificate expiry (`getRefreshInterval()`)
- Signals processes when certificates are renewed

**Impact:** Without rotation, certificates will expire and services will fail.

### 2. Bundle Writing (Not Implemented)

**Current state:** Only writes the leaf certificate and private key. No bundle file.

**Go implementation:**
- Writes trust bundle to `SVIDBundleFilename`
- Optionally includes federated domain certificates
- Optionally moves intermediates from cert chain to bundle
- Filters expired certificates when `omit_expired` is true

### 3. Process Management (Not Implemented)

**Current state:** No support for launching or signaling processes.

**Go implementation:**
- `cmd` + `cmd_args`: Launches a child process on first certificate fetch
- `renew_signal`: Sends signal (e.g., SIGHUP) to process on certificate renewal
- `pid_file_name`: Reads PID from file to signal external processes
- Monitors child process exit and cleans up

### 4. JWT Support (Not Implemented)

**Current state:** Config struct defines JWT fields but no implementation.

**Go implementation:**
- `jwt_svids`: Array of audience/filename pairs for JWT SVID fetching
- `jwt_bundle_file_name`: Writes JWT bundle set in JSON format
- Continuous JWT SVID refresh with expiry-based intervals
- Separate watchers for JWT bundles and SVIDs

### 5. Health Check Accuracy (Partial)

**Current state:** Health endpoints always return 200 OK.

**Go implementation:**
- Tracks write status for each credential type (x509, jwt_bundle, jwt_svids)
- Liveness: Returns 503 if any write operation failed
- Readiness: Returns 503 until all configured credentials are written
- Health response includes detailed status per credential

---

## Implementation Phases

### Phase 1: Core Certificate Lifecycle (Essential)

These features are required for production use.

#### 1.1 X.509 Certificate Rotation
- Implement `WatchX509Context` using the spiffe crate's watcher API
- Calculate refresh interval as half the time until expiry
- Write new certificates to disk when updates arrive
- Track certificate state for health checks

#### 1.2 X.509 Bundle Writing
- Add `svid_bundle_file_name` configuration support
- Write trust bundle in PEM format
- Include all CA certificates from the X509Context

#### 1.3 Accurate Health Status
- Track write success/failure for X.509 credentials
- Return 503 on liveness if writes failed
- Return 503 on readiness until initial write succeeds
- Include detailed status in health response body

### Phase 2: Process Integration (Important)

These features enable integration with applications that need certificate reload signals.

#### 2.1 Renewal Signal Support
- Implement `renew_signal` configuration (e.g., "SIGHUP", "SIGUSR1")
- Parse signal names to actual signal numbers
- Send signal to process after certificate renewal

#### 2.2 PID File Support
- Implement `pid_file_name` configuration
- Read PID from file on each signal attempt
- Retry logic for race conditions during process startup

#### 2.3 Command Execution
- Implement `cmd` and `cmd_args` configuration
- Parse `cmd_args` respecting quoted strings
- Launch child process after initial certificate fetch
- Monitor child process for exit
- Send renewal signals instead of restarting

#### 2.4 File Permission Modes
- Implement `cert_file_mode` (default: 0644)
- Implement `key_file_mode` (default: 0600)
- Apply permissions when writing files

### Phase 3: JWT Support (Feature Complete)

These features add JWT SVID support for JWT-based authentication.

#### 3.1 JWT SVID Fetching
- Implement `jwt_svids` array configuration
- Fetch JWT SVIDs for each configured audience
- Support `jwt_extra_audiences` for additional audiences
- Write JWT SVID to configured filename

#### 3.2 JWT Bundle Fetching
- Implement `jwt_bundle_file_name` configuration
- Fetch JWT bundle set from Workload API
- Write bundle in JSON format (trust domain -> base64 keys)
- Implement `jwt_bundle_file_mode` permission

#### 3.3 JWT SVID Rotation
- Watch for JWT SVID expiry
- Refresh at half the time until expiry
- Implement `jwt_svid_file_mode` permission
- Track JWT health status

### Phase 4: Advanced Certificate Options (Nice to Have)

These features provide advanced control over certificate handling.

#### 4.1 Intermediate Certificate Handling
- Implement `add_intermediates_to_bundle` configuration
- When true: move intermediates from cert file to bundle file
- When false (default): keep intermediates in cert file

#### 4.2 Federated Domain Support
- Implement `include_federated_domains` configuration
- When true: include federated trust domains in bundle
- Requires changes to bundle writing logic

#### 4.3 Certificate Filtering
- Implement `omit_expired` configuration
- Filter expired certificates from bundle
- Check NotAfter field against current time

#### 4.4 SVID Hint Selection
- Implement `hint` configuration
- Pass hint to Workload API for SVID selection
- Use hint when multiple SVIDs are available

### Phase 5: Platform Support (Optional)

#### 5.1 Windows Compatibility
- Handle lack of Unix signals on Windows
- Document Windows limitations
- Consider alternative notification mechanisms

---

## Detailed Task Breakdown

### Phase 1 Tasks

```
[ ] 1.1.1 Add X509Context watcher using spiffe::workloadapi::X509Source
[ ] 1.1.2 Implement callback for X509Context updates
[ ] 1.1.3 Calculate refresh interval from certificate NotAfter
[ ] 1.1.4 Update daemon event loop to handle certificate updates
[ ] 1.1.5 Add tests for certificate rotation logic

[ ] 1.2.1 Add svid_bundle_file_name to active config usage
[ ] 1.2.2 Extract trust bundle from X509Context
[ ] 1.2.3 Write bundle in PEM format with CA certificates
[ ] 1.2.4 Add tests for bundle writing

[ ] 1.3.1 Create HealthStatus struct to track credential states
[ ] 1.3.2 Update workload_api to report success/failure
[ ] 1.3.3 Implement CheckLiveness() - false if write failed
[ ] 1.3.4 Implement CheckReadiness() - false until written
[ ] 1.3.5 Return HealthStatus in health response body
[ ] 1.3.6 Add tests for health status logic
```

### Phase 2 Tasks

```
[ ] 2.1.1 Add signal name parsing (SIGHUP, SIGUSR1, etc.)
[ ] 2.1.2 Implement signal sending to process
[ ] 2.1.3 Integrate signal sending with certificate update flow
[ ] 2.1.4 Add tests for signal handling

[ ] 2.2.1 Implement PID file reading
[ ] 2.2.2 Add retry logic for PID file operations
[ ] 2.2.3 Send signals to PID from file
[ ] 2.2.4 Add tests for PID file support

[ ] 2.3.1 Implement cmd_args parsing (handle quoted strings)
[ ] 2.3.2 Spawn child process using tokio::process
[ ] 2.3.3 Monitor child process for exit
[ ] 2.3.4 Switch from spawn to signal on subsequent updates
[ ] 2.3.5 Add tests for process management

[ ] 2.4.1 Add file mode configuration parsing
[ ] 2.4.2 Apply permissions in certificate write functions
[ ] 2.4.3 Add tests for file permissions
```

### Phase 3 Tasks

```
[ ] 3.1.1 Implement JWT SVID fetching via Workload API
[ ] 3.1.2 Support multiple audiences per JWT config
[ ] 3.1.3 Write JWT SVID to configured file
[ ] 3.1.4 Add tests for JWT SVID fetching

[ ] 3.2.1 Implement JWT bundle fetching via Workload API
[ ] 3.2.2 Serialize bundle to JSON format
[ ] 3.2.3 Write bundle with configured permissions
[ ] 3.2.4 Add tests for JWT bundle handling

[ ] 3.3.1 Add JWT SVID watcher
[ ] 3.3.2 Calculate JWT refresh intervals
[ ] 3.3.3 Track JWT health status
[ ] 3.3.4 Add tests for JWT rotation
```

### Phase 4 Tasks

```
[ ] 4.1.1 Implement intermediate certificate extraction
[ ] 4.1.2 Conditionally add intermediates to bundle
[ ] 4.1.3 Add tests for intermediate handling

[ ] 4.2.1 Fetch federated trust domains
[ ] 4.2.2 Include federated certs in bundle
[ ] 4.2.3 Add tests for federated domains

[ ] 4.3.1 Implement certificate expiry checking
[ ] 4.3.2 Filter expired certs from bundle
[ ] 4.3.3 Add tests for expiry filtering

[ ] 4.4.1 Pass hint to Workload API calls
[ ] 4.4.2 Select SVID by hint when multiple available
[ ] 4.4.3 Add tests for hint selection
```

---

## Configuration Reference

### Currently Implemented

```hcl
agent_address = "unix:///run/spire/sockets/workload_api.sock"
cert_dir = "/certs"
daemon_mode = true
svid_file_name = "svid.pem"
svid_key_file_name = "svid_key.pem"

health_checks {
  listener_enabled = true
  bind_port = 8080
  liveness_path = "/health/live"
  readiness_path = "/health/ready"
}
```

### To Be Implemented

```hcl
# Phase 1
svid_bundle_file_name = "bundle.pem"

# Phase 2
cmd = "/usr/bin/nginx"
cmd_args = "-c /etc/nginx/nginx.conf"
pid_file_name = "/run/nginx.pid"
renew_signal = "SIGHUP"
cert_file_mode = 0644
key_file_mode = 0600

# Phase 3
jwt_bundle_file_name = "jwt_bundle.json"
jwt_bundle_file_mode = 0644
jwt_svid_file_mode = 0644
jwt_svids = [
  {
    jwt_audience = "my-service"
    jwt_extra_audiences = ["other-service"]
    jwt_svid_file_name = "jwt_svid.token"
  }
]

# Phase 4
add_intermediates_to_bundle = false
include_federated_domains = false
omit_expired = false
hint = ""
```

---

## Success Criteria

### Phase 1 Complete
- Certificates automatically rotate before expiry
- Trust bundle is written alongside certificates
- Health endpoints reflect actual certificate status

### Phase 2 Complete
- Can launch and signal processes on certificate renewal
- Can signal external processes via PID file
- File permissions are correctly applied

### Phase 3 Complete
- JWT SVIDs are fetched and rotated
- JWT bundles are written in correct JSON format
- JWT health status is tracked

### Phase 4 Complete
- All advanced certificate options work
- Full feature parity with Go implementation

---

## Testing Strategy

### Unit Tests
- Configuration parsing for all new fields
- Signal name parsing
- Certificate expiry calculation
- PID file parsing
- JWT serialization

### Integration Tests
- Certificate rotation with mock SPIFFE server
- Process signal delivery
- Health endpoint behavior
- End-to-end with kind + SPIRE (existing infrastructure)

### Manual Testing
- Deploy to kind cluster with real SPIRE
- Verify certificate rotation
- Test process signaling with nginx
- Validate health checks during rotation

---

## Estimated Complexity

| Phase | Complexity | Key Challenges |
|-------|------------|----------------|
| Phase 1 | High | Async watcher integration, state management |
| Phase 2 | Medium | Unix signal handling, process lifecycle |
| Phase 3 | Medium | JWT API differences, JSON serialization |
| Phase 4 | Low | Straightforward configuration additions |

---

## Dependencies

Current dependencies are sufficient for Phase 1-2. Additional dependencies may be needed:

- **Phase 2**: `nix` crate for Unix signal handling
- **Phase 3**: `serde_json` for JWT bundle serialization (may already be available via serde)

---

## References

- [spiffe-helper Go source](https://github.com/spiffe/spiffe-helper)
- [spiffe-rs crate documentation](https://docs.rs/spiffe)
- [SPIFFE Workload API specification](https://github.com/spiffe/spiffe/blob/main/standards/SPIFFE_Workload_API.md)
