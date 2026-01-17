#!/bin/bash

# Get the directory of this script
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
ROOT_DIR="$( cd "$DIR/.." && pwd )"

# Build the mock agent
echo "Building SPIRE Agent Mock..."
cargo build -p spire-agent-mock

# Default socket path
SOCKET_PATH=${SPIFFE_ENDPOINT_SOCKET:-/tmp/agent.sock}

echo "Starting SPIRE Agent Mock on $SOCKET_PATH..."
# Run the mock agent
exec "$ROOT_DIR/target/debug/spire-agent-mock" --socket-path "$SOCKET_PATH"
