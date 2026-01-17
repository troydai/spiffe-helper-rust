# Problem 001: Mock Spire Agent

## Problem Statement

The integration test is done through a complete setup of SPIRE server and
agent in a local kind cluster. This setup is a good representation of the
production environment.

Creation of this environment is, however, slow. The turnaround time of
creating cluster, loading images, waiting for server and agent to start takes
minutes if not tens of minutes. When running this setup locally and CI/CD
leads to prolonged turnaround time and slows down the development velocity.

## Solution

The spiffe-helper-rust relies on spire-agent's workload API to function.
The API implementation can be mocked by implementing a spire-agent that functions
with given instructions without a server. This mock agent can be run locally
outside of a cluster. Additionally this agent can simulate failure modes that
are otherwise hard to reproduce through a real spire-agent.

## Execution Plan

### Phase 1: Project Restructuring (Workspace) - [Done]
To support the separation of the main application and the test tool, we will convert the project into a Cargo Workspace.

1.  [x] **Create Workspace**: Establish a root-level `Cargo.toml` configured as a Virtual Manifest for the workspace.
2.  [x] **Migrate Main App**: Move the existing `spiffe-helper-rust` code (src, existing Cargo.toml, etc.) into a subdirectory named `spiffe-helper`.
3.  [x] **Create Mock Crate**: Initialize a new crate `spire-mock` in a sibling directory.
4.  [x] **Isolate Tests**: Ensure existing integration tests (which run against the cluster) are preserved but decoupled from the default fast feedback loop.

### Phase 2: Mock Agent Implementation (`spire-mock`) - [Done]
The mock agent will be a standalone binary that implements the SPIFFE Workload API.

1.  [x] **Dependencies**: Configure `spire-mock` with `tonic` (gRPC), `prost`, and `tokio`.
2.  [x] **Protocol Definition**:
    *   Fetch the official `workload.proto` from the SPIFFE repository.
    *   Configure `build.rs` to generate the server-side Rust code.
3.  [x] **Server Implementation**:
    *   Implement the `SpiffeWorkloadApiService` trait.
    *   Create a gRPC server listening on a Unix Domain Socket (UDS).
4.  [x] **Mock Logic (Initial)**:
    *   **Data Source**: The mock will load pre-generated SVIDs (X.509 certificates) and Private Keys from a local directory provided via CLI arguments. This avoids complex on-the-fly CA logic for the first iteration.
    *   **Behavior**: Respond to `FetchX509SVIDRequest` with the loaded bundle.

### Phase 3: Integration Testing - [Done]
Develop a fast integration test suite using the mock.

1.  [x] **Test Fixture**: Create a test harness that:
    *   Launches `spire-mock` in the background with a specific socket path.
    *   Runs `spiffe-helper` targeting that socket.
    *   Verifies that `spiffe-helper` correctly fetches and saves the certificates.
2.  [x] **Cluster Tests**: Retain the `kind`-based tests but likely move them to a separate workflow or mark them (e.g., `#[ignore]`) so they are run explicitly (e.g., `cargo test -- --include-ignored` or via a dedicated script) rather than on every save.

## Agent Execution Instruction

This section is for AI coding agents that execute this plan. Agents that execute this plan have different roles: orchestrator and worker.

For orchestrator:
- Read the entire plan and memorize its end goal;
- Launch worker agent to execute the plan sequentially;
- Feed instruction to worker and verify their work;
- Communicate with worker to understand its progress;
- Once a worker is done its job, mark the task down in this document and launch a new worker to start next job.
- It is important to record the progress in this file and commit through git, this allows another orchestrator agent to pick up the remaining work when necessary.

For worker:
- Read the entire plan but focus on the task that is assigned to you by the orchestrator;
- Work only for that task nothing else;
- Code and test the work;
- Commit the work in git and create PR waiting for review;
