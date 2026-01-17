# Problem 001: Mock Spire Agent

## Problem Statement

The integration test is done through a complete set up of SPIRE server and
agent in a local kind cluster. This set up is a good representation of the
production environment.

Creation of this environment is, however, slow. The turn around time of
creating cluster, load images, waiting for server and agent to start takes
minutes if not tens of minutes. When running this set up locally and CI/CD
leads to prolong turn around time and slow down the development velocity.

## Solution

The spiffe-helper-rust relies on spire-agent's workload API to function.
The API implement can be mocked by implement a spire-agent that function
with given instruction without a server. This mock agent can be run locally
outside of a cluster. Additionally this agent can perform failure mode that
nevertheless hard to reproduce through a real spire-agent.

## Execution Plan

### Phase 1: Project Restructuring (Workspace)
To support the separation of the main application and the test tool, we will convert the project into a Cargo Workspace.

1.  **Create Workspace**: Establish a root-level `Cargo.toml` configured as a Virtual Manifest for the workspace.
2.  **Migrate Main App**: Move the existing `spiffe-helper-rust` code (src, existing Cargo.toml, etc.) into a subdirectory named `spiffe-helper`.
3.  **Create Mock Crate**: Initialize a new crate `spire-mock` in a sibling directory.
4.  **Isolate Tests**: Ensure existing integration tests (which run against the cluster) are preserved but decoupled from the default fast feedback loop.

### Phase 2: Mock Agent Implementation (`spire-mock`)
The mock agent will be a standalone binary that implements the SPIFFE Workload API.

1.  **Dependencies**: Configure `spire-mock` with `tonic` (gRPC), `prost`, and `tokio`.
2.  **Protocol Definition**:
    *   Fetch the official `workload.proto` from the SPIFFE repository.
    *   Configure `build.rs` to generate the server-side Rust code.
3.  **Server Implementation**:
    *   Implement the `SpiffeWorkloadApiService` trait.
    *   Create a gRPC server listening on a Unix Domain Socket (UDS).
4.  **Mock Logic (Initial)**:
    *   **Data Source**: The mock will load pre-generated SVIDs (X.509 certificates) and Private Keys from a local directory provided via CLI arguments. This avoids complex on-the-fly CA logic for the first iteration.
    *   **Behavior**: Respond to `FetchX509SVIDRequest` with the loaded bundle.

### Phase 3: Integration Testing
Develop a fast integration test suite using the mock.

1.  **Test Fixture**: Create a test harness that:
    *   Launches `spire-mock` in the background with a specific socket path.
    *   Runs `spiffe-helper` targeting that socket.
    *   Verifies that `spiffe-helper` correctly fetches and saves the certificates.
2.  **Cluster Tests**: Retain the `kind`-based tests but likely move them to a separate workflow or mark them (e.g., `#[ignore]`) so they are run explicitly (e.g., `cargo test -- --include-ignored` or via a dedicated script) rather than on every save.
