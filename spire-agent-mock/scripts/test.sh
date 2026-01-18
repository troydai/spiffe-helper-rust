#!/bin/bash

# Get the directory of this script
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$DIR/.." && pwd )"

# Default socket path
SOCKET_PATH=${SPIFFE_ENDPOINT_SOCKET:-/tmp/agent.sock}

if [ ! -S "$SOCKET_PATH" ]; then
    echo "Error: Socket $SOCKET_PATH not found. Is the mock agent running?"
    exit 1
fi

# grpcurl on this system works best with the unix:///path/to/socket URI scheme
UDS_URI="unix://$SOCKET_PATH"

echo "Testing FetchX509SVID..."
grpcurl -plaintext \
  -import-path "$PROJECT_ROOT/proto" \
  -proto "$PROJECT_ROOT/proto/workload.proto" \
  -d '{}' \
  "$UDS_URI" \
  SpiffeWorkloadAPI/FetchX509SVID

echo -e "\nTesting FetchJWTSVID..."
grpcurl -plaintext \
  -import-path "$PROJECT_ROOT/proto" \
  -proto "$PROJECT_ROOT/proto/workload.proto" \
  -d '{"audience": ["my-service"]}' \
  "$UDS_URI" \
  SpiffeWorkloadAPI/FetchJWTSVID