# Integration Test Documentation

## Entry Organization

The SPIRE workload registration system uses a hierarchical entry organization to efficiently manage workload identities across a Kubernetes cluster.

### Node Alias Pattern

The registration system employs a **node alias** pattern to optimize entry creation. Instead of creating separate entries for each agent-workload combination (which would result in O(n×m) entries), the system creates a single node alias entry that represents all agents in the cluster.

#### Node Alias Entry

- **SPIFFE ID**: `spiffe://spiffe-helper.local/k8s-cluster/spiffe-helper`
- **Type**: Node entry (created with the `-node` flag)
- **Selector**: `k8s_psat:cluster:spiffe-helper`
- **Purpose**: Acts as a parent identity for all workload entries, representing all SPIRE agents in the cluster that belong to the `spiffe-helper` cluster

This node alias is created first and serves as a common parent for workload entries, eliminating the need to create individual entries for each agent.

### Workload Entry Format

Workload entries follow a structured format with three components:

1. **SPIFFE ID**: The unique identity assigned to the workload
   - Format: `spiffe://spiffe-helper.local/ns/<namespace>/sa/<service-account>`
   - Example: `spiffe://spiffe-helper.local/ns/default/sa/spiffe-helper-test`

2. **Parent ID**: The parent identity that issued this workload's identity
   - Can contain a wildcard (`*`) which is automatically replaced with the node alias ID
   - Format: `spiffe://spiffe-helper.local/spire/agent/k8s_psat/spiffe-helper/*`
   - When processed, the wildcard is replaced with: `spiffe://spiffe-helper.local/k8s-cluster/spiffe-helper`

3. **Selectors**: Comma-separated list of matching criteria used to identify workloads
   - Format: `k8s:ns:<namespace>,k8s:sa:<service-account>`
   - Example: `k8s:ns:default,k8s:sa:spiffe-helper-test`
   - These selectors match Kubernetes namespaces and service accounts

### Example Workload Registrations

The system registers the following sample workloads:

1. **Default Namespace Test Workload**
   - SPIFFE ID: `spiffe://spiffe-helper.local/ns/default/sa/spiffe-helper-test`
   - Parent: Node alias (via wildcard)
   - Selectors: `k8s:ns:default,k8s:sa:spiffe-helper-test`

2. **Spiffe-Helper Namespace Test Workload**
   - SPIFFE ID: `spiffe://spiffe-helper.local/ns/spiffe-helper/sa/test-workload`
   - Parent: Node alias (via wildcard)
   - Selectors: `k8s:ns:spiffe-helper,k8s:sa:test-workload`

3. **Httpbin Workload**
   - SPIFFE ID: `spiffe://spiffe-helper.local/ns/httpbin/sa/httpbin`
   - Parent: Node alias (via wildcard)
   - Selectors: `k8s:ns:httpbin,k8s:sa:httpbin`

### Benefits of This Organization

1. **Scalability**: The node alias pattern reduces entry count from O(n×m) to O(n), where n is the number of workloads and m is the number of agents.

2. **Simplicity**: Workload entries can use wildcards in parent IDs, automatically resolving to the node alias without manual per-agent configuration.

3. **Consistency**: All workloads in the cluster share the same parent identity (the node alias), creating a uniform trust hierarchy.

4. **Maintainability**: Adding new agents doesn't require updating existing workload entries, as they all reference the cluster-wide node alias.

### Registration Process

1. **Validation**: The script checks that the SPIRE server is running and ready
2. **Node Alias Creation**: Ensures the node alias entry exists (creates if missing)
3. **Workload Registration**: Processes each workload entry:
   - Checks if the entry already exists (skips if present)
   - Resolves wildcard parent IDs to the node alias
   - Parses comma-separated selectors into individual selector flags
   - Creates the entry via the SPIRE server API
4. **Verification**: Lists all registered entries for confirmation
