#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo "[undeploy] Removing SPIRE workload registration controller..."

# Optionally deregister entries if SPIRE server is still running
if kubectl get namespace spire-server > /dev/null 2>&1; then
	SPIRE_SERVER_POD=$(kubectl get pods -n spire-server -l app=spire-server -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")
	if [ -n "${SPIRE_SERVER_POD}" ]; then
		echo "[undeploy] Attempting to deregister workload entries..."
		
		# Get workloads from ConfigMap if it exists
		if kubectl get configmap spire-registration-config -n spire-registration > /dev/null 2>&1; then
			WORKLOADS=$(kubectl get configmap spire-registration-config -n spire-registration -o jsonpath='{.data.workloads}' 2>/dev/null || echo "")
			
			# Deregister entries
			while IFS= read -r line; do
				# Skip empty lines and comments
				[[ -z "${line// }" ]] && continue
				[[ "${line}" =~ ^[[:space:]]*#.*$ ]] && continue
				
				# Parse the line: spiffe_id|parent_id|selectors
				IFS='|' read -r spiffe_id parent_id selectors <<< "${line}"
				spiffe_id=$(echo "${spiffe_id}" | xargs)
				
				if [ -n "${spiffe_id}" ]; then
					echo "[undeploy] Deregistering entries for: ${spiffe_id}"
					# Get all entry IDs for this SPIFFE ID (there may be multiple entries for different agents)
					ENTRY_IDS=$(kubectl exec -n spire-server "${SPIRE_SERVER_POD}" -- \
						/opt/spire/bin/spire-server entry show -spiffeID "${spiffe_id}" 2>/dev/null | \
						grep "Entry ID" | awk '{print $4}' || echo "")
					
					if [ -n "${ENTRY_IDS}" ]; then
						# Delete each entry ID
						while IFS= read -r entry_id; do
							[[ -z "${entry_id}" ]] && continue
							echo "[undeploy] Deleting entry ID: ${entry_id}"
							kubectl exec -n spire-server "${SPIRE_SERVER_POD}" -- \
								/opt/spire/bin/spire-server entry delete -entryID "${entry_id}" 2>/dev/null || true
						done <<< "${ENTRY_IDS}"
						echo "[undeploy] Deregistered all entries for: ${spiffe_id}"
					else
						echo "[undeploy] No entries found for ${spiffe_id}, skipping..."
					fi
				fi
			done <<< "${WORKLOADS}"
		fi
	fi
fi

# Delete registration resources
if kubectl get namespace spire-registration > /dev/null 2>&1; then
	echo "[undeploy] Deleting ConfigMap..."
	kubectl delete configmap spire-registration-config -n spire-registration --ignore-not-found=true
	
	echo "[undeploy] Deleting ServiceAccount..."
	kubectl delete serviceaccount spire-registration -n spire-registration --ignore-not-found=true
	
	echo "[undeploy] Deleting ClusterRoleBinding and ClusterRole..."
	kubectl delete clusterrolebinding spire-registration-cluster-role-binding --ignore-not-found=true
	kubectl delete clusterrole spire-registration-cluster-role --ignore-not-found=true
	
	echo "[undeploy] Deleting namespace..."
	kubectl delete namespace spire-registration --ignore-not-found=true
	
	echo "[undeploy] SPIRE workload registration controller removed successfully!"
else
	echo "[undeploy] Namespace 'spire-registration' does not exist. Nothing to remove."
fi
