#!/usr/bin/env bash
# Removes the SPIRE agent and its associated resources from the cluster.
set -euo pipefail

# Source color support
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../colors.sh"

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo -e "${COLOR_BRIGHT_BLUE}[undeploy-spire-agent]${COLOR_RESET} ${COLOR_BOLD}Removing SPIRE agent...${COLOR_RESET}"

if [ ! -f "${KUBECONFIG_PATH}" ]; then
	echo -e "${COLOR_YELLOW}[undeploy-spire-agent]${COLOR_RESET} Kubeconfig not found. Skipping undeploy."
	exit 0
fi

echo -e "${COLOR_CYAN}[undeploy-spire-agent]${COLOR_RESET} Deleting DaemonSet..."
kubectl delete daemonset spire-agent -n spire-agent --ignore-not-found=true
echo -e "${COLOR_CYAN}[undeploy-spire-agent]${COLOR_RESET} Deleting ConfigMap..."
kubectl delete configmap spire-agent-config -n spire-agent --ignore-not-found=true
echo -e "${COLOR_CYAN}[undeploy-spire-agent]${COLOR_RESET} Deleting ClusterRoleBinding and ClusterRole..."
kubectl delete clusterrolebinding spire-agent-cluster-role-binding --ignore-not-found=true
kubectl delete clusterrole spire-agent-cluster-role --ignore-not-found=true
echo -e "${COLOR_CYAN}[undeploy-spire-agent]${COLOR_RESET} Deleting ServiceAccount..."
kubectl delete serviceaccount spire-agent -n spire-agent --ignore-not-found=true
echo -e "${COLOR_CYAN}[undeploy-spire-agent]${COLOR_RESET} Deleting Secret..."
kubectl delete secret spire-bundle -n spire-agent --ignore-not-found=true
echo -e "${COLOR_CYAN}[undeploy-spire-agent]${COLOR_RESET} Deleting namespace..."
kubectl delete namespace spire-agent --ignore-not-found=true

echo ""
echo -e "${COLOR_BRIGHT_GREEN}[undeploy-spire-agent]${COLOR_RESET} ${COLOR_BOLD}SPIRE agent removed.${COLOR_RESET}"
