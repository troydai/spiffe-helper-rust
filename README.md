# spiffe-helper-rust

A Rust implementation of spiffe-helper.

spiffe-helper fetches SPIFFE X.509 certificates and JWT tokens from the SPIRE agent. It acts as a bridge to integrate other programs with SPIRE.

## Configuration

spiffe-helper-rust uses an HCL configuration file (default: `helper.conf`) to configure its behavior.

### Operation Modes

spiffe-helper-rust supports two operation modes controlled by the `daemon_mode` configuration option. **Daemon mode is the default** when `daemon_mode` is not specified.

#### Daemon Mode (`daemon_mode = true`, **default**)

When `daemon_mode` is set to `true` (or not specified), the program runs continuously until it receives a SIGTERM signal. This mode is suitable for sidecar containers that need to run alongside the main application:

- **X.509 Certificate Fetching**: On startup, the daemon connects to the SPIRE agent and fetches the X.509 SVID (certificate and private key). The certificates are written to the configured output directory before the daemon continues. If certificate fetching fails, the daemon exits with code 1.
- The program keeps running until SIGTERM is received
- Periodic liveness logs are printed every 30 seconds to demonstrate the daemon is running
- Health check endpoints can be enabled for Kubernetes probes
- The program shuts down gracefully when SIGTERM is received

**Use case**: Sidecar containers that need to continuously fetch and update certificates.

#### One-Shot Mode (`daemon_mode = false`)

When `daemon_mode` is set to `false`, the program fetches certificates once and exits. This mode is suitable for initContainers:

- Fetches certificates once and exits successfully
- Creates the certificate directory if needed
- Main container starts after initContainer completes

**Use case**: InitContainers that fetch certificates before the main container starts.

#### Configuration

The mode can be set in two ways:

1. **Via configuration file:**
   ```hcl
   daemon_mode = true   # default, can be omitted
   # or
   daemon_mode = false  # for one-shot mode
   ```

   **Note:** If `daemon_mode` is not specified, daemon mode is used by default.

2. **Via command-line flag:**
   ```bash
   spiffe-helper-rust --daemon-mode true --config helper.conf
   # or
   spiffe-helper-rust --daemon-mode false --config helper.conf
   ```

   The command-line flag overrides the configuration file setting.

### Health Checks

Health checks can be configured to support Kubernetes liveness and readiness probes. When enabled, an HTTP server is started to serve health check endpoints.

#### Configuration

Health checks are configured in the `health_checks` block:

```hcl
health_checks {
    listener_enabled = true
    bind_port = 8080
    liveness_path = "/health/live"
    readiness_path = "/health/ready"
}
```

**Configuration Options:**

- `listener_enabled` (boolean, required): Enable or disable the health check HTTP server
- `bind_port` (integer, default: 8080): Port number to bind the health check server (0-65535)
- `liveness_path` (string, default: "/health/live"): HTTP path for liveness probe
- `readiness_path` (string, default: "/health/ready"): HTTP path for readiness probe

**Note:** If `listener_enabled` is `false`, the health check server is not started, and other health check settings are ignored.

#### Health Check Endpoints

When health checks are enabled, the following endpoints are available:

- **Liveness Probe**: Returns HTTP 200 OK to indicate the daemon is alive
- **Readiness Probe**: Returns HTTP 200 OK to indicate the daemon is ready

Both endpoints return a simple HTTP 200 status code. The paths can be customized via the configuration file.

#### Example Kubernetes Configuration

```yaml
livenessProbe:
  httpGet:
    path: /health/live
    port: 8080
  initialDelaySeconds: 10
  periodSeconds: 30

readinessProbe:
  httpGet:
    path: /health/ready
    port: 8080
  initialDelaySeconds: 5
  periodSeconds: 10
```

### Signal Handling

In daemon mode, the program responds to the following signals:

- **SIGTERM**: Initiates graceful shutdown. The program will:
  - Stop accepting new health check requests
  - Shut down the health check server
  - Exit cleanly

## Usage

### Running in Daemon Mode (Default)

```bash
# Using configuration file (daemon mode is the default)
spiffe-helper-rust --config helper.conf

# Explicitly enabling daemon mode via command line
spiffe-helper-rust --config helper.conf --daemon-mode true

# Running in one-shot mode (explicitly disable daemon mode)
spiffe-helper-rust --config helper.conf --daemon-mode false

# Stopping the daemon
kill -TERM <pid>
```

### X.509 Certificate Fetching

In daemon mode, spiffe-helper-rust automatically fetches X.509 SVIDs (certificates and private keys) from the SPIRE agent at startup. The certificates are persisted to the configured output directory.

#### Configuration

The following configuration options control X.509 certificate fetching:

- `agent_address` (string, required for daemon mode): Address of the SPIRE agent Workload API (e.g., `"unix:///tmp/agent.sock"` or `"unix:///run/spire/sockets/workload_api.sock"`)
- `cert_dir` (string, required for daemon mode): Directory where certificates will be written
- `svid_file_name` (string, optional): Filename for the X.509 certificate (default: `"svid.pem"`)
- `svid_key_file_name` (string, optional): Filename for the X.509 private key (default: `"svid_key.pem"`)

#### Behavior

- **Startup**: When daemon mode starts, it immediately attempts to fetch the X.509 certificate and key from the SPIRE agent
- **Success**: If fetching succeeds, certificates are written to the configured directory and the daemon continues running
- **Failure**: If fetching fails (e.g., agent unavailable, connection error), the daemon exits with code 1, ensuring initContainers fail if certificates cannot be obtained

This ensures that certificates are available before the main application container starts, making it suitable for use in Kubernetes initContainers.

### Example Configuration File

```hcl
agent_address = "unix:///tmp/agent.sock"
# daemon_mode = true  # Optional: daemon mode is the default
cert_dir = "/etc/certs"
svid_file_name = "svid.pem"
svid_key_file_name = "svid_key.pem"

health_checks {
    listener_enabled = true
    bind_port = 8080
    liveness_path = "/health/live"
    readiness_path = "/health/ready"
}
```

**Note:** The `daemon_mode` setting is optional. If omitted, daemon mode is used by default. To use one-shot mode, explicitly set `daemon_mode = false`.

## Integration Testing

This repository includes a comprehensive integration test environment using a local kind cluster with SPIRE server and agents. For detailed instructions on setting up and using the integration test environment, see [Integration Test Documentation](docs/integration_test.md).

The integration test environment includes:
- Certificate generation for testing
- Local kind cluster setup
- SPIRE server and agent deployment
- Environment orchestration and validation

To get started quickly:

```bash
# Set up the entire integration test environment
make env-up

# Run smoke tests to validate the environment
make smoke-test

# Tear down the environment
make env-down
```

For more details, see the [Integration Test Documentation](docs/integration_test.md).
