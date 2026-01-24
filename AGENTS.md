# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

spiffe-helper-rust is a Rust implementation of spiffe-helper. It fetches SPIFFE X.509 certificates and JWT tokens from the SPIRE agent, acting as a bridge to integrate other programs with SPIRE. The tool supports two operation modes: daemon mode (continuous running with health checks) and one-shot mode (fetch once and exit, suitable for initContainers).

## Build and Development Commands

```bash
# Build
cargo build

# Run tests
cargo test

# Run a single test
cargo test <test_name>

# Lint
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings

# Build Docker image
make build-helper-image

# Pre-commit hooks (install once)
pre-commit install

# Run pre-commit manually on all files
pre-commit run --all-files
```

## Integration Testing

The project includes a comprehensive integration test environment using a local kind cluster with SPIRE:

```bash
# Full environment setup (builds images, creates kind cluster, deploys SPIRE)
make env-up

# Run smoke tests
make smoke-test

# Tear down environment
make env-down

# Individual components
make cluster-up      # Create kind cluster only
make cluster-down    # Delete kind cluster
make certs           # Generate test certificates
make deploy-spire-server
make deploy-spire-agent
make load-images     # Load Docker images into kind cluster
```

## Architecture

### Source Structure (src/)

- **main.rs**: Entry point with CLI argument parsing (clap), daemon/one-shot mode selection, health check HTTP server (axum), and SIGTERM signal handling
- **config.rs**: HCL configuration file parser using hcl-rs crate, defines `Config` and `HealthChecksConfig` structs
- **workload_api.rs**: SPIFFE Workload API client that fetches X.509 SVIDs from the SPIRE agent with retry logic and exponential backoff

### Key Dependencies

- `spiffe`: SPIFFE Workload API client
- `axum`: HTTP server for health check endpoints
- `clap`: CLI argument parsing
- `hcl-rs`: HCL configuration file parsing
- `tokio`: Async runtime

### Configuration

Uses HCL format configuration files (default: `helper.conf`). Key settings:
- `agent_address`: SPIRE agent socket (e.g., `"unix:///run/spire/sockets/workload_api.sock"`)
- `cert_dir`: Output directory for certificates
- `daemon_mode`: true (default) for continuous running, false for one-shot
- `health_checks`: Optional block for HTTP health endpoints

### Operation Modes

1. **Daemon mode** (default): Fetches initial certificate, starts health check server if configured, runs until SIGTERM
2. **One-shot mode**: Fetches certificate once and exits (for Kubernetes initContainers)

## Workflow: Working on GitHub Issues

When asked to work on a specific GitHub issue, follow this sequence:

1. **Create branch and worktree**
   ```bash
   # Use gwt to create and manage the worktree for the branch.
   # It prints the worktree path (shell integration may auto-cd).
   gwt sw -b <branch_name>
   cd <printed_path>
   ```

2. **Read the GitHub issue carefully and form a plan**
   - Use `gh issue view <issue_number>` to read the issue details
   - Analyze requirements and acceptance criteria
   - Create an implementation plan

3. **Execute the plan**
   - Implement the changes
   - Run tests: `cargo test`
   - Run lints: `cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings`

4. **Create PR**
   - Commit changes with descriptive message
   - Push branch and create PR: `gh pr create`
