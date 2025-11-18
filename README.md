# spiffe-helper-rust

A Rust implementation of spiffe-helper.

spiffe-helper fetches SPIFFE X.509 certificates and JWT tokens from the SPIRE agent. It acts as a bridge to integrate other programs with SPIRE.

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
