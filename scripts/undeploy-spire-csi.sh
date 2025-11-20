#!/usr/bin/env bash
set -euo pipefail

# Source color support
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/colors.sh"

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo -e "${COLOR_BRIGHT_BLUE}[undeploy-spire-csi]${COLOR_RESET} ${COLOR_BOLD}Removing SPIRE CSI driver...${COLOR_RESET}"

if kubectl get namespace spire-csi > /dev/null 2>&1; then
	echo -e "${COLOR_CYAN}[undeploy-spire-csi]${COLOR_RESET} Deleting DaemonSet..."
	kubectl delete daemonset spire-csi-driver -n spire-csi --ignore-not-found=true
	echo -e "${COLOR_CYAN}[undeploy-spire-csi]${COLOR_RESET} Deleting CSIDriver..."
	kubectl delete csidriver spire-csi --ignore-not-found=true
	echo -e "${COLOR_CYAN}[undeploy-spire-csi]${COLOR_RESET} Deleting ConfigMap..."
	kubectl delete configmap spire-csi-driver-config -n spire-csi --ignore-not-found=true
	echo -e "${COLOR_CYAN}[undeploy-spire-csi]${COLOR_RESET} Deleting ServiceAccount..."
	kubectl delete serviceaccount spire-csi-driver -n spire-csi --ignore-not-found=true
	echo -e "${COLOR_CYAN}[undeploy-spire-csi]${COLOR_RESET} Deleting ClusterRoleBinding and ClusterRole..."
	kubectl delete clusterrolebinding spire-csi-driver-cluster-role-binding --ignore-not-found=true
	kubectl delete clusterrole spire-csi-driver-cluster-role --ignore-not-found=true
	echo -e "${COLOR_CYAN}[undeploy-spire-csi]${COLOR_RESET} Deleting namespace..."
	kubectl delete namespace spire-csi --ignore-not-found=true
	echo ""
	echo -e "${COLOR_BRIGHT_GREEN}[undeploy-spire-csi]${COLOR_RESET} ${COLOR_BOLD}SPIRE CSI driver removed successfully!${COLOR_RESET}"
else
	echo -e "${COLOR_YELLOW}[undeploy-spire-csi]${COLOR_RESET} Namespace '${COLOR_BOLD}spire-csi${COLOR_RESET}' does not exist. Nothing to remove."
fi
