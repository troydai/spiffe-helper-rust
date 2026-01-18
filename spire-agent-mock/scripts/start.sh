#!/bin/bash

# Get the directory of this script
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
# Workspace root is two levels up from spire-agent-mock/scripts/
WORKSPACE_ROOT="$( cd "$DIR/../.." && pwd )"

# Build the mock agent
echo "Building SPIRE Agent Mock..."
cargo build -p spire-agent-mock

# Default socket path
SOCKET_PATH=${SPIFFE_ENDPOINT_SOCKET:-/tmp/agent.sock}

echo "Starting SPIRE Agent Mock on $SOCKET_PATH..."
# Run the mock agent from the workspace target directory
exec "$WORKSPACE_ROOT/target/debug/spire-agent-mock" --socket-path "$SOCKET_PATH"