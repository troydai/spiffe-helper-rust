# SPIRE Agent Mock

A mock SPIRE agent that implements the SPIFFE Workload API for testing purposes.

## Configuration

- `--socket-path` (env: `SPIRE_MOCK_SOCKET_PATH`): UDS path to listen on.
- `--x509-internal` (env: `SPIRE_MOCK_X509_INTERNAL_SECONDS`): X.509 SVID rotation interval in seconds (default: 30).
