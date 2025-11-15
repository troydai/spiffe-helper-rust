#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"
DEPLOY_DIR="${DEPLOY_DIR:-${ROOT_DIR}/deploy/spire/registration}"

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

echo "[deploy] Creating namespace..."
kubectl apply -f "${DEPLOY_DIR}/namespace.yaml"

echo "[deploy] Creating ConfigMap..."
kubectl apply -f "${DEPLOY_DIR}/configmap.yaml"

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

# Function to get all attested agent IDs
get_agent_ids() {
	kubectl exec -n spire-server "${SPIRE_SERVER_POD}" -- \
		/opt/spire/bin/spire-server agent list 2>/dev/null | \
		grep "SPIFFE ID" | awk '{print $4}' || echo ""
}

# Function to register a workload entry for a specific parent (agent)
register_entry_for_parent() {
	local spiffe_id="$1"
	local parent_id="$2"
	local selectors="$3"
	
	# Convert comma-separated selectors to multiple -selector flags
	local selector_flags=""
	IFS=',' read -ra SELECTOR_ARRAY <<< "${selectors}"
	for selector in "${SELECTOR_ARRAY[@]}"; do
		selector_flags="${selector_flags} -selector ${selector}"
	done
	
	# Create the entry
	if kubectl exec -n spire-server "${SPIRE_SERVER_POD}" -- \
		/opt/spire/bin/spire-server entry create \
		-spiffeID "${spiffe_id}" \
		-parentID "${parent_id}" \
		${selector_flags} 2>/dev/null; then
		return 0
	else
		return 1
	fi
}

# Function to register a workload entry (handles wildcard parent IDs)
register_entry() {
	local spiffe_id="$1"
	local parent_id="$2"
	local selectors="$3"
	
	echo "[registration] Registering entry: ${spiffe_id}"
	
	# Check if entry already exists (for any parent)
	if kubectl exec -n spire-server "${SPIRE_SERVER_POD}" -- \
		/opt/spire/bin/spire-server entry show -spiffeID "${spiffe_id}" 2>/dev/null | grep -q "${spiffe_id}"; then
		echo "[registration] Entry ${spiffe_id} already exists, skipping..."
		return 0
	fi
	
	# If parent_id contains a wildcard, register for all agents
	if [[ "${parent_id}" == *"*"* ]]; then
		echo "[registration] Parent ID contains wildcard, discovering agents..."
		local agent_ids
		agent_ids=$(get_agent_ids)
		
		if [ -z "${agent_ids}" ]; then
			echo "[registration] Warning: No attested agents found. Cannot register entry."
			return 1
		fi
		
		# Extract the base parent ID pattern (everything before the *)
		local parent_base="${parent_id%%\**}"
		local parent_suffix="${parent_id#*\*}"
		
		local registered_count=0
		while IFS= read -r agent_id; do
			[[ -z "${agent_id}" ]] && continue
			
			# For k8s_psat, the agent ID format is: spiffe://<trust-domain>/spire/agent/k8s_psat/<cluster>/<node-id>
			# We can use the agent_id directly as the parent
			if register_entry_for_parent "${spiffe_id}" "${agent_id}" "${selectors}"; then
				echo "[registration] Successfully registered entry ${spiffe_id} for agent ${agent_id}"
				((registered_count++)) || true
			else
				echo "[registration] Warning: Failed to register entry ${spiffe_id} for agent ${agent_id}"
			fi
		done <<< "${agent_ids}"
		
		if [ "${registered_count}" -gt 0 ]; then
			return 0
		else
			return 1
		fi
	else
		# Parent ID is specific, register directly
		if register_entry_for_parent "${spiffe_id}" "${parent_id}" "${selectors}"; then
			echo "[registration] Successfully registered entry: ${spiffe_id}"
			return 0
		else
			echo "[registration] Warning: Failed to create entry ${spiffe_id}"
			return 1
		fi
	fi
}

# Register sample workloads from ConfigMap
echo "[deploy] Registering sample workloads..."
TRUST_DOMAIN=$(kubectl get configmap spire-registration-config -n spire-registration -o jsonpath='{.data.trust_domain}')
WORKLOADS=$(kubectl get configmap spire-registration-config -n spire-registration -o jsonpath='{.data.workloads}')

echo "[deploy] Trust domain: ${TRUST_DOMAIN}"

# Parse workloads from ConfigMap (format: spiffe_id|parent_id|selectors)
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
