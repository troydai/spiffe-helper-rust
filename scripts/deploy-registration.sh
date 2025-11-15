#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo "[deploy] Deploying SPIRE workload registration controller..."

# Check if SPIRE server is running
echo "[deploy] Checking SPIRE server status..."
if ! kubectl get pods -n spire-server -l app=spire-server --field-selector=status.phase=Running 2>/dev/null | grep -q Running; then
	echo "[deploy] Error: SPIRE server is not running. Please deploy it first with 'make deploy-spire-server'"
	exit 1
fi

# Check if SPIRE agent is running (optional but recommended)
if ! kubectl get pods -n spire-agent -l app=spire-agent --field-selector=status.phase=Running 2>/dev/null | grep -q Running; then
	echo "[deploy] Warning: SPIRE agent is not running. Workloads may not be able to attest."
fi

# Wait for SPIRE server to be ready
echo "[deploy] Waiting for SPIRE server to be ready..."
SPIRE_SERVER_POD=$(kubectl get pods -n spire-server -l app=spire-server -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")
if [ -z "${SPIRE_SERVER_POD}" ]; then
	echo "[deploy] Error: Could not find SPIRE server pod"
	exit 1
fi

echo "[deploy] Found SPIRE server pod: ${SPIRE_SERVER_POD}"
kubectl wait --for=condition=ready pod/"${SPIRE_SERVER_POD}" -n spire-server --timeout=60s || {
	echo "[deploy] Warning: SPIRE server pod may not be fully ready"
}

# Node alias SPIFFE ID (represents all agents in the cluster)
NODE_ALIAS_ID="spiffe://spiffe-helper.local/k8s-cluster/spiffe-helper"

# Function to ensure node alias exists
ensure_node_alias() {
	echo "[registration] Ensuring node alias exists..."
	
	# Check if node alias already exists
	if kubectl exec -n spire-server "${SPIRE_SERVER_POD}" -- \
		/opt/spire/bin/spire-server entry show -spiffeID "${NODE_ALIAS_ID}" 2>/dev/null | grep -q "${NODE_ALIAS_ID}"; then
		echo "[registration] Node alias ${NODE_ALIAS_ID} already exists"
		return 0
	fi
	
	# Create node registration entry (node alias) for all agents in the cluster
	# This uses k8s_psat cluster selector to match all agents
	if kubectl exec -n spire-server "${SPIRE_SERVER_POD}" -- \
		/opt/spire/bin/spire-server entry create \
		-node \
		-spiffeID "${NODE_ALIAS_ID}" \
		-selector "k8s_psat:cluster:spiffe-helper" 2>/dev/null; then
		echo "[registration] Created node alias: ${NODE_ALIAS_ID}"
		return 0
	else
		echo "[registration] Warning: Failed to create node alias, but continuing..."
		return 1
	fi
}

# Function to register a workload entry
register_entry() {
	local spiffe_id="$1"
	local parent_id="$2"
	local selectors="$3"
	
	echo "[registration] Registering entry: ${spiffe_id}"
	
	# Check if entry already exists
	if kubectl exec -n spire-server "${SPIRE_SERVER_POD}" -- \
		/opt/spire/bin/spire-server entry show -spiffeID "${spiffe_id}" 2>/dev/null | grep -q "${spiffe_id}"; then
		echo "[registration] Entry ${spiffe_id} already exists, skipping..."
		return 0
	fi
	
	# Convert comma-separated selectors to multiple -selector flags
	local selector_flags=""
	IFS=',' read -ra SELECTOR_ARRAY <<< "${selectors}"
	for selector in "${SELECTOR_ARRAY[@]}"; do
		selector_flags="${selector_flags} -selector ${selector}"
	done
	
	# If parent_id contains a wildcard, use node alias instead
	if [[ "${parent_id}" == *"*"* ]]; then
		parent_id="${NODE_ALIAS_ID}"
		echo "[registration] Using node alias as parent: ${parent_id}"
	fi
	
	# Create the entry
	if kubectl exec -n spire-server "${SPIRE_SERVER_POD}" -- \
		/opt/spire/bin/spire-server entry create \
		-spiffeID "${spiffe_id}" \
		-parentID "${parent_id}" \
		${selector_flags} 2>/dev/null; then
		echo "[registration] Successfully registered entry: ${spiffe_id}"
		return 0
	else
		echo "[registration] Warning: Failed to create entry ${spiffe_id}"
		return 1
	fi
}

# Ensure node alias exists (represents all agents in the cluster)
ensure_node_alias

# Register sample workloads
echo "[deploy] Registering sample workloads..."

# Sample workload registrations
# Format: spiffe_id|parent_id|selectors (comma-separated)
# Parent ID with * wildcard will be replaced with node alias (O(n) instead of O(n*m))
# Selectors format: k8s:ns:<namespace> for namespace, k8s:sa:<service-account> for service account
WORKLOADS=$(cat <<'EOF'
spiffe://spiffe-helper.local/ns/default/sa/spiffe-helper-test|spiffe://spiffe-helper.local/spire/agent/k8s_psat/spiffe-helper/*|k8s:ns:default,k8s:sa:spiffe-helper-test
spiffe://spiffe-helper.local/ns/spiffe-helper/sa/test-workload|spiffe://spiffe-helper.local/spire/agent/k8s_psat/spiffe-helper/*|k8s:ns:spiffe-helper,k8s:sa:test-workload
spiffe://spiffe-helper.local/ns/httpbin/sa/httpbin|spiffe://spiffe-helper.local/spire/agent/k8s_psat/spiffe-helper/*|k8s:ns:httpbin,k8s:sa:httpbin
EOF
)

# Parse workloads (format: spiffe_id|parent_id|selectors)
REGISTRATION_COUNT=0
while IFS= read -r line; do
	# Skip empty lines and comments
	[[ -z "${line// }" ]] && continue
	[[ "${line}" =~ ^[[:space:]]*#.*$ ]] && continue
	
	# Parse the line: spiffe_id|parent_id|selectors
	IFS='|' read -r spiffe_id parent_id selectors <<< "${line}"
	
	# Trim whitespace
	spiffe_id=$(echo "${spiffe_id}" | xargs)
	parent_id=$(echo "${parent_id}" | xargs)
	selectors=$(echo "${selectors}" | xargs)
	
	if register_entry "${spiffe_id}" "${parent_id}" "${selectors}"; then
		((REGISTRATION_COUNT++)) || true
	fi
done <<< "${WORKLOADS}"

echo "[deploy] Registered ${REGISTRATION_COUNT} workload entries"

# Show registered entries
echo "[deploy] Listing registered entries..."
kubectl exec -n spire-server "${SPIRE_SERVER_POD}" -- \
	/opt/spire/bin/spire-server entry show || {
	echo "[deploy] Warning: Could not list entries"
}

echo "[deploy] SPIRE workload registration complete!"
