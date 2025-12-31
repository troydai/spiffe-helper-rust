#!/usr/bin/env bash
# Registers workload entries in the SPIRE server for sample services.
set -euo pipefail

# Source color support
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../colors.sh"

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo -e "${COLOR_BRIGHT_BLUE}[deploy]${COLOR_RESET} ${COLOR_BOLD}Deploying SPIRE workload registration controller...${COLOR_RESET}"

# Check if SPIRE server is running
echo -e "${COLOR_CYAN}[deploy]${COLOR_RESET} Checking SPIRE server status..."
if ! kubectl get pods -n spire-server -l app=spire-server --field-selector=status.phase=Running 2>/dev/null | grep -q Running; then
	echo -e "${COLOR_RED}[deploy] Error:${COLOR_RESET} SPIRE server is not running. Please deploy it first with 'make deploy-spire-server'"
	exit 1
fi

# Check if SPIRE agent is running (optional but recommended)
if ! kubectl get pods -n spire-agent -l app=spire-agent --field-selector=status.phase=Running 2>/dev/null | grep -q Running; then
	echo -e "${COLOR_YELLOW}[deploy]${COLOR_RESET} Warning: SPIRE agent is not running. Workloads may not be able to attest."
fi

# Wait for SPIRE server to be ready
echo -e "${COLOR_CYAN}[deploy]${COLOR_RESET} Waiting for SPIRE server to be ready..."
SPIRE_SERVER_POD=$(kubectl get pods -n spire-server -l app=spire-server -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")
if [ -z "${SPIRE_SERVER_POD}" ]; then
	echo -e "${COLOR_RED}[deploy] Error:${COLOR_RESET} Could not find SPIRE server pod"
	exit 1
fi

echo -e "${COLOR_GREEN}[deploy]${COLOR_RESET} Found SPIRE server pod: ${COLOR_BOLD}${SPIRE_SERVER_POD}${COLOR_RESET}"
kubectl wait --for=condition=ready pod/"${SPIRE_SERVER_POD}" -n spire-server --timeout=60s || {
	echo -e "${COLOR_YELLOW}[deploy]${COLOR_RESET} Warning: SPIRE server pod may not be fully ready"
}

# Node alias SPIFFE ID (represents all agents in the cluster)
NODE_ALIAS_ID="spiffe://spiffe-helper.local/k8s-cluster/spiffe-helper"

# Function to ensure node alias exists
ensure_node_alias() {
	echo -e "${COLOR_CYAN}[registration]${COLOR_RESET} Ensuring node alias exists..."
	
	# Check if node alias already exists
	if kubectl exec -n spire-server "${SPIRE_SERVER_POD}" -- \
		/opt/spire/bin/spire-server entry show -spiffeID "${NODE_ALIAS_ID}" 2>/dev/null | grep -q "${NODE_ALIAS_ID}"; then
		echo -e "${COLOR_YELLOW}[registration]${COLOR_RESET} Node alias ${COLOR_CYAN}${NODE_ALIAS_ID}${COLOR_RESET} already exists"
		return 0
	fi
	
	# Create node registration entry (node alias) for all agents in the cluster
	# This uses k8s_psat cluster selector to match all agents
	if kubectl exec -n spire-server "${SPIRE_SERVER_POD}" -- \
		/opt/spire/bin/spire-server entry create \
		-node \
		-spiffeID "${NODE_ALIAS_ID}" \
		-selector "k8s_psat:cluster:spiffe-helper" 2>/dev/null; then
		echo -e "${COLOR_GREEN}✓${COLOR_RESET} Created node alias: ${COLOR_CYAN}${NODE_ALIAS_ID}${COLOR_RESET}"
		return 0
	else
		echo -e "${COLOR_YELLOW}[registration]${COLOR_RESET} Warning: Failed to create node alias, but continuing..."
		return 1
	fi
}

# Function to register a workload entry
register_entry() {
	local spiffe_id="$1"
	local parent_id="$2"
	local selectors="$3"
	
	echo -e "${COLOR_CYAN}[registration]${COLOR_RESET} Registering entry: ${COLOR_BOLD}${spiffe_id}${COLOR_RESET}"
	
	# Check if entry already exists
	if kubectl exec -n spire-server "${SPIRE_SERVER_POD}" -- \
		/opt/spire/bin/spire-server entry show -spiffeID "${spiffe_id}" 2>/dev/null | grep -q "${spiffe_id}"; then
		echo -e "${COLOR_YELLOW}[registration]${COLOR_RESET} Entry ${COLOR_CYAN}${spiffe_id}${COLOR_RESET} already exists, skipping..."
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
		echo -e "${COLOR_CYAN}[registration]${COLOR_RESET} Using node alias as parent: ${COLOR_CYAN}${parent_id}${COLOR_RESET}"
	fi
	
	# Create the entry
	if kubectl exec -n spire-server "${SPIRE_SERVER_POD}" -- \
		/opt/spire/bin/spire-server entry create \
		-spiffeID "${spiffe_id}" \
		-parentID "${parent_id}" \
		${selector_flags} 2>/dev/null; then
		echo -e "${COLOR_GREEN}✓${COLOR_RESET} Successfully registered entry: ${COLOR_CYAN}${spiffe_id}${COLOR_RESET}"
		return 0
	else
		echo -e "${COLOR_YELLOW}[registration]${COLOR_RESET} Warning: Failed to create entry ${COLOR_CYAN}${spiffe_id}${COLOR_RESET}"
		return 1
	fi
}

# Ensure node alias exists (represents all agents in the cluster)
ensure_node_alias

# Register sample workloads
echo -e "${COLOR_CYAN}[deploy]${COLOR_RESET} Registering sample workloads..."

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

echo -e "${COLOR_GREEN}[deploy]${COLOR_RESET} Registered ${COLOR_BOLD}${REGISTRATION_COUNT}${COLOR_RESET} workload entries"

# Show registered entries
echo -e "${COLOR_CYAN}[deploy]${COLOR_RESET} Listing registered entries..."
kubectl exec -n spire-server "${SPIRE_SERVER_POD}" -- \
	/opt/spire/bin/spire-server entry show || {
	echo -e "${COLOR_YELLOW}[deploy]${COLOR_RESET} Warning: Could not list entries"
}

echo ""
echo -e "${COLOR_BRIGHT_GREEN}[deploy]${COLOR_RESET} ${COLOR_BOLD}SPIRE workload registration complete!${COLOR_RESET}"
