#!/bin/bash

# Get the directory of this script
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
# Workspace root is two levels up from spire-agent-mock/scripts/
WORKSPACE_ROOT="$( cd "$DIR/../.." && pwd )"

# Build the mock agent
echo "Building SPIRE Agent Mock..."
cargo build -p spire-agent-mock

# Default socket path
SOCKET_PATH=${SPIRE_MOCK_SOCKET_PATH:-/tmp/agent.sock}
ROTATION_SECONDS=${SPIRE_MOCK_X509_INTERNAL_SECONDS:-30}

echo "Starting SPIRE Agent Mock on $SOCKET_PATH (rotation ${ROTATION_SECONDS}s)..."
# Run the mock agent from the workspace target directory
exec "$WORKSPACE_ROOT/target/debug/spire-agent-mock" \
  --socket-path "$SOCKET_PATH" \
  --x509-internal "$ROTATION_SECONDS"
