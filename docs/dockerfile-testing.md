# Dockerfile Testing Guide

This document describes how to test the Dockerfile changes for `dumb-init` integration.

## Changes Made

The Dockerfile has been updated to use `dumb-init` as the entrypoint for proper signal handling:

1. **Installed `dumb-init`**: Added to the runtime stage installation
2. **Updated `ENTRYPOINT`**: Changed to `dumb-init --`
3. **Added `CMD`**: Set to run the actual binary

## Validation

Run the validation script to verify the Dockerfile structure:

```bash
./scripts/validate-dockerfile.sh
```

## Manual Testing

### 1. Build the Docker Image

```bash
docker build -t spiffe-helper-rust:test .
```

### 2. Verify dumb-init is Installed

```bash
docker run --rm spiffe-helper-rust:test dumb-init --version
```

Expected output: Version information for dumb-init

### 3. Verify Entrypoint Configuration

```bash
# Start a container and check PID 1
docker run -d --name test-container spiffe-helper-rust:test --version
docker exec test-container ps -p 1 -o comm=
docker rm -f test-container
```

Expected output: `dumb-init` (not `spiffe-helper-rust`)

### 4. Test SIGTERM Signal Handling (Daemon Mode)

Create a test config file:

```bash
cat > /tmp/test-helper.conf <<EOF
agent_address = "unix:///tmp/agent.sock"
daemon_mode = true
cert_dir = "/tmp/certs"
EOF
```

Start container in daemon mode:

```bash
docker run -d --name test-daemon \
    -v /tmp/test-helper.conf:/app/helper.conf:ro \
    spiffe-helper-rust:test --config /app/helper.conf
```

Wait for daemon to start, then send SIGTERM:

```bash
sleep 2
docker stop -t 10 test-daemon
```

Check logs for graceful shutdown:

```bash
docker logs test-daemon
```

Expected output should include:
- "Starting spiffe-helper-rust daemon..."
- "Daemon running. Waiting for SIGTERM to shutdown..."
- "Received SIGTERM, shutting down gracefully..."
- "Health check server stopped"
- "Daemon shutdown complete"

Verify exit code:

```bash
docker inspect test-daemon --format='{{.State.ExitCode}}'
```

Expected output: `0` (clean exit)

Cleanup:

```bash
docker rm test-daemon
rm /tmp/test-helper.conf
```

### 5. Automated Testing

Run the comprehensive test script:

```bash
./scripts/test-dockerfile.sh
```

This script will:
- Build the Docker image
- Verify dumb-init installation
- Verify entrypoint configuration
- Test SIGTERM handling in daemon mode
- Verify graceful shutdown

## Integration Testing

The Docker image is used in Kubernetes deployments. To test in a real environment:

1. Build and load the image into kind:
   ```bash
   docker build -t spiffe-helper-rust:test .
   kind load docker-image spiffe-helper-rust:test --name spiffe-helper
   ```

2. Deploy httpbin with spiffe-helper:
   ```bash
   kubectl apply -f deploy/httpbin/httpbin.yaml
   ```

3. Check initContainer logs:
   ```bash
   kubectl logs -n httpbin <pod-name> -c spiffe-helper
   ```

4. Test daemon mode by updating the config to `daemon_mode = true` and checking signal handling

## Benefits Verified

- ✅ Proper signal handling: SIGTERM is forwarded correctly
- ✅ Zombie process reaping: dumb-init handles child processes
- ✅ Graceful shutdown: Daemon responds to SIGTERM
- ✅ Container best practices: Follows recommended patterns
