#!/usr/bin/env bash
# Deregisters sample workload entries from the SPIRE server.
set -euo pipefail

# Source color support
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../utility/colors.sh"

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo -e "${COLOR_BRIGHT_BLUE}[undeploy]${COLOR_RESET} ${COLOR_BOLD}Removing SPIRE workload registration controller...${COLOR_RESET}"

# Node alias SPIFFE ID
NODE_ALIAS_ID="spiffe://spiffe-helper.local/k8s-cluster/spiffe-helper"

# Optionally deregister entries if SPIRE server is still running
if kubectl get namespace spire-server > /dev/null 2>&1; then
	SPIRE_SERVER_POD=$(kubectl get pods -n spire-server -l app=spire-server -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")
	if [ -n "${SPIRE_SERVER_POD}" ]; then
		echo -e "${COLOR_CYAN}[undeploy]${COLOR_RESET} Attempting to deregister workload entries..."
		
		# Sample workload registrations (same as in deploy script)
		WORKLOADS=$(cat <<'EOF'
spiffe://spiffe-helper.local/ns/default/sa/spiffe-helper-test|spiffe://spiffe-helper.local/spire/agent/k8s_psat/spiffe-helper/*|k8s:ns:default,k8s:sa:spiffe-helper-test
spiffe://spiffe-helper.local/ns/spiffe-helper/sa/test-workload|spiffe://spiffe-helper.local/spire/agent/k8s_psat/spiffe-helper/*|k8s:ns:spiffe-helper,k8s:sa:test-workload
spiffe://spiffe-helper.local/ns/httpbin/sa/httpbin|spiffe://spiffe-helper.local/spire/agent/k8s_psat/spiffe-helper/*|k8s:ns:httpbin,k8s:sa:httpbin
EOF
)
		
		# Deregister entries
		while IFS= read -r line; do
			# Skip empty lines and comments
			[[ -z "${line// }" ]] && continue
			[[ "${line}" =~ ^[[:space:]]*#.*$ ]] && continue
			
			# Parse the line: spiffe_id|parent_id|selectors
			IFS='|' read -r spiffe_id parent_id selectors <<< "${line}"
			spiffe_id=$(echo "${spiffe_id}" | xargs)
			
			if [ -n "${spiffe_id}" ]; then
				echo -e "${COLOR_CYAN}[undeploy]${COLOR_RESET} Deregistering entries for: ${COLOR_BOLD}${spiffe_id}${COLOR_RESET}"
				# Get all entry IDs for this SPIFFE ID (there may be multiple entries for different agents)
				ENTRY_IDS=$(kubectl exec -n spire-server "${SPIRE_SERVER_POD}" -- \
					/opt/spire/bin/spire-server entry show -spiffeID "${spiffe_id}" 2>/dev/null | \
					grep "Entry ID" | awk '{print $4}' || echo "")
				
				if [ -n "${ENTRY_IDS}" ]; then
					# Delete each entry ID
					while IFS= read -r entry_id; do
						[[ -z "${entry_id}" ]] && continue
						echo -e "${COLOR_CYAN}[undeploy]${COLOR_RESET} Deleting entry ID: ${COLOR_CYAN}${entry_id}${COLOR_RESET}"
						kubectl exec -n spire-server "${SPIRE_SERVER_POD}" -- \
							/opt/spire/bin/spire-server entry delete -entryID "${entry_id}" 2>/dev/null || true
					done <<< "${ENTRY_IDS}"
					echo -e "${COLOR_GREEN}✓${COLOR_RESET} Deregistered all entries for: ${COLOR_CYAN}${spiffe_id}${COLOR_RESET}"
				else
					echo -e "${COLOR_YELLOW}[undeploy]${COLOR_RESET} No entries found for ${COLOR_CYAN}${spiffe_id}${COLOR_RESET}, skipping..."
				fi
			fi
		done <<< "${WORKLOADS}"
		
		# Optionally delete node alias (only if no workloads are using it)
		echo -e "${COLOR_CYAN}[undeploy]${COLOR_RESET} Checking if node alias should be removed..."
		# Note: We'll leave the node alias in place as it may be used by other entries
	fi
fi

echo ""
echo -e "${COLOR_BRIGHT_GREEN}[undeploy]${COLOR_RESET} ${COLOR_BOLD}SPIRE workload registration cleanup complete!${COLOR_RESET}"
