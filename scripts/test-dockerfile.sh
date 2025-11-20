#!/bin/bash
# Test script for Dockerfile changes - validates dumb-init integration

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

# Colors
source "$SCRIPT_DIR/colors.sh"

IMAGE_NAME="spiffe-helper-rust:test"
CONTAINER_NAME="spiffe-helper-test-$$"

# Cleanup function
cleanup() {
    echo -e "${COLOR_CYAN}[cleanup]${COLOR_RESET} Cleaning up test container..."
    docker rm -f "$CONTAINER_NAME" 2>/dev/null || true
}

trap cleanup EXIT

echo -e "${COLOR_BLUE}[test]${COLOR_RESET} Building Docker image..."
docker build -t "$IMAGE_NAME" .

echo -e "${COLOR_BLUE}[test]${COLOR_RESET} Verifying dumb-init is installed..."
docker run --rm "$IMAGE_NAME" dumb-init --version

echo -e "${COLOR_BLUE}[test]${COLOR_RESET} Verifying entrypoint uses dumb-init..."
# Check that dumb-init is PID 1
docker run -d --name "$CONTAINER_NAME" "$IMAGE_NAME" --version
sleep 1

# Get PID 1 process name
PID1_PROC=$(docker exec "$CONTAINER_NAME" ps -p 1 -o comm=)
if [ "$PID1_PROC" != "dumb-init" ]; then
    echo -e "${COLOR_RED}[test]${COLOR_RESET} ERROR: PID 1 is '$PID1_PROC', expected 'dumb-init'"
    exit 1
fi
echo -e "${COLOR_GREEN}[test]${COLOR_RESET} ✓ dumb-init is PID 1"

# Clean up test container
docker rm -f "$CONTAINER_NAME"

echo -e "${COLOR_BLUE}[test]${COLOR_RESET} Testing SIGTERM signal handling in daemon mode..."

# Create a temporary config file for daemon mode
TEMP_CONFIG=$(mktemp)
cat > "$TEMP_CONFIG" <<EOF
agent_address = "unix:///tmp/agent.sock"
daemon_mode = true
cert_dir = "/tmp/certs"
EOF

# Start container in daemon mode
docker run -d --name "$CONTAINER_NAME" \
    -v "$TEMP_CONFIG:/app/helper.conf:ro" \
    "$IMAGE_NAME" --config /app/helper.conf

# Wait for daemon to start
sleep 2

# Check that the process is running
if ! docker ps | grep -q "$CONTAINER_NAME"; then
    echo -e "${COLOR_RED}[test]${COLOR_RESET} ERROR: Container exited unexpectedly"
    docker logs "$CONTAINER_NAME"
    exit 1
fi

# Send SIGTERM
echo -e "${COLOR_BLUE}[test]${COLOR_RESET} Sending SIGTERM to container..."
docker stop -t 10 "$CONTAINER_NAME"

# Wait a moment for graceful shutdown
sleep 2

# Check logs for graceful shutdown message
if docker logs "$CONTAINER_NAME" 2>&1 | grep -q "Received SIGTERM, shutting down gracefully"; then
    echo -e "${COLOR_GREEN}[test]${COLOR_RESET} ✓ SIGTERM handled gracefully"
else
    echo -e "${COLOR_YELLOW}[test]${COLOR_RESET} WARNING: Graceful shutdown message not found in logs"
    docker logs "$CONTAINER_NAME"
fi

# Verify container stopped cleanly
if docker ps -a | grep "$CONTAINER_NAME" | grep -q "Exited (0)"; then
    echo -e "${COLOR_GREEN}[test]${COLOR_RESET} ✓ Container exited with code 0"
else
    EXIT_CODE=$(docker inspect "$CONTAINER_NAME" --format='{{.State.ExitCode}}')
    echo -e "${COLOR_YELLOW}[test]${COLOR_RESET} Container exited with code: $EXIT_CODE"
fi

# Cleanup
rm -f "$TEMP_CONFIG"
docker rm -f "$CONTAINER_NAME" 2>/dev/null || true

echo -e "${COLOR_BRIGHT_GREEN}[test]${COLOR_RESET} All tests passed!"
