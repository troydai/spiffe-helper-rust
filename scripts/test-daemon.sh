#!/bin/bash
set -e

# Test script for daemon mode functionality
# This script tests:
# 1. Daemon starts correctly
# 2. Health check endpoints work
# 3. Periodic logging works
# 4. SIGTERM handling works

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARY="$ROOT_DIR/target/release/spiffe-helper-rust"
TEST_DIR=$(mktemp -d)
CONFIG_FILE="$TEST_DIR/helper.conf"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

cleanup() {
    echo -e "${YELLOW}Cleaning up...${NC}"
    if [ -n "$DAEMON_PID" ]; then
        kill -TERM "$DAEMON_PID" 2>/dev/null || true
        wait "$DAEMON_PID" 2>/dev/null || true
    fi
    rm -rf "$TEST_DIR"
}

trap cleanup EXIT

echo -e "${GREEN}=== Testing Daemon Mode ===${NC}"

# Check if binary exists
if [ ! -f "$BINARY" ]; then
    echo -e "${RED}Error: Binary not found at $BINARY${NC}"
    echo "Please run 'cargo build --release' first"
    exit 1
fi

# Create test config file
cat > "$CONFIG_FILE" <<EOF
agent_address = "unix:///tmp/test-agent.sock"
daemon_mode = true
cert_dir = "$TEST_DIR/certs"

health_checks {
    listener_enabled = true
    bind_port = 8080
    liveness_path = "/health/live"
    readiness_path = "/health/ready"
}
EOF

echo -e "${GREEN}[1/4] Starting daemon...${NC}"
cd "$TEST_DIR"
"$BINARY" --config "$CONFIG_FILE" > daemon.log 2>&1 &
DAEMON_PID=$!

# Wait for daemon to start
sleep 2

# Check if daemon is running
if ! kill -0 "$DAEMON_PID" 2>/dev/null; then
    echo -e "${RED}Error: Daemon failed to start${NC}"
    cat daemon.log
    exit 1
fi

echo -e "${GREEN}[2/4] Testing health check endpoints...${NC}"
sleep 1

# Test liveness endpoint
LIVENESS_CODE=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/health/live || echo "000")
if [ "$LIVENESS_CODE" != "200" ]; then
    echo -e "${RED}Error: Liveness endpoint returned $LIVENESS_CODE, expected 200${NC}"
    cat daemon.log
    exit 1
fi
echo -e "${GREEN}  ✓ Liveness endpoint: HTTP $LIVENESS_CODE${NC}"

# Test readiness endpoint
READINESS_CODE=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/health/ready || echo "000")
if [ "$READINESS_CODE" != "200" ]; then
    echo -e "${RED}Error: Readiness endpoint returned $READINESS_CODE, expected 200${NC}"
    cat daemon.log
    exit 1
fi
echo -e "${GREEN}  ✓ Readiness endpoint: HTTP $READINESS_CODE${NC}"

echo -e "${GREEN}[3/4] Testing periodic logging...${NC}"
# Wait for at least one log message
sleep 35

if ! grep -q "spiffe-helper-rust daemon is alive" daemon.log; then
    echo -e "${RED}Error: Periodic log message not found${NC}"
    cat daemon.log
    exit 1
fi
echo -e "${GREEN}  ✓ Periodic logging working${NC}"

echo -e "${GREEN}[4/4] Testing SIGTERM handling...${NC}"
# Send SIGTERM
kill -TERM "$DAEMON_PID"

# Wait for graceful shutdown (max 5 seconds)
for i in {1..5}; do
    if ! kill -0 "$DAEMON_PID" 2>/dev/null; then
        break
    fi
    sleep 1
done

# Check if process terminated
if kill -0 "$DAEMON_PID" 2>/dev/null; then
    echo -e "${RED}Error: Process did not terminate after SIGTERM${NC}"
    kill -9 "$DAEMON_PID" 2>/dev/null || true
    exit 1
fi

# Check logs for shutdown message
if grep -q "Received SIGTERM" daemon.log && grep -q "Daemon shutdown complete" daemon.log; then
    echo -e "${GREEN}  ✓ Graceful shutdown successful${NC}"
else
    echo -e "${YELLOW}  ⚠ Shutdown message not found in logs (may be normal)${NC}"
    cat daemon.log
fi

echo -e "${GREEN}=== All Tests Passed! ===${NC}"
