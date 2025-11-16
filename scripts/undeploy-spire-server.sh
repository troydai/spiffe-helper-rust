#!/usr/bin/env bash
set -euo pipefail

# Source color support
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/colors.sh"

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo -e "${COLOR_BRIGHT_BLUE}[undeploy]${COLOR_RESET} ${COLOR_BOLD}Removing SPIRE server...${COLOR_RESET}"
if kubectl get namespace spire-server > /dev/null 2>&1; then
	echo -e "${COLOR_CYAN}[undeploy]${COLOR_RESET} Deleting StatefulSet..."
	kubectl delete statefulset spire-server -n spire-server --ignore-not-found=true
	echo -e "${COLOR_CYAN}[undeploy]${COLOR_RESET} Deleting Service..."
	kubectl delete service spire-server -n spire-server --ignore-not-found=true
	echo -e "${COLOR_CYAN}[undeploy]${COLOR_RESET} Deleting ConfigMap..."
	kubectl delete configmap spire-server-config -n spire-server --ignore-not-found=true
	echo -e "${COLOR_CYAN}[undeploy]${COLOR_RESET} Deleting Secrets..."
	kubectl delete secret spire-server-tls spire-server-ca spire-server-bootstrap -n spire-server --ignore-not-found=true
	echo -e "${COLOR_CYAN}[undeploy]${COLOR_RESET} Deleting ServiceAccount..."
	kubectl delete serviceaccount spire-server -n spire-server --ignore-not-found=true
	echo -e "${COLOR_CYAN}[undeploy]${COLOR_RESET} Deleting ClusterRoleBinding and ClusterRole..."
	kubectl delete clusterrolebinding spire-server-cluster-role-binding --ignore-not-found=true
	kubectl delete clusterrole spire-server-cluster-role --ignore-not-found=true
	echo -e "${COLOR_CYAN}[undeploy]${COLOR_RESET} Deleting namespace..."
	kubectl delete namespace spire-server --ignore-not-found=true
	echo ""
	echo -e "${COLOR_BRIGHT_GREEN}[undeploy]${COLOR_RESET} ${COLOR_BOLD}SPIRE server removed successfully!${COLOR_RESET}"
else
	echo -e "${COLOR_YELLOW}[undeploy]${COLOR_RESET} Namespace '${COLOR_BOLD}spire-server${COLOR_RESET}' does not exist. Nothing to remove."
fi

