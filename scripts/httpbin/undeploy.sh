#!/usr/bin/env bash
# Removes the httpbin service and its associated resources from the cluster.
set -euo pipefail

# Source color support
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../colors.sh"

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo -e "${COLOR_BRIGHT_BLUE}[undeploy]${COLOR_RESET} ${COLOR_BOLD}Removing httpbin service...${COLOR_RESET}"
if kubectl get namespace httpbin > /dev/null 2>&1; then
	echo -e "${COLOR_CYAN}[undeploy]${COLOR_RESET} Deleting Deployment..."
	kubectl delete deployment httpbin -n httpbin --ignore-not-found=true
	echo -e "${COLOR_CYAN}[undeploy]${COLOR_RESET} Deleting Service..."
	kubectl delete service httpbin -n httpbin --ignore-not-found=true
	echo -e "${COLOR_CYAN}[undeploy]${COLOR_RESET} Deleting ConfigMap..."
	kubectl delete configmap spiffe-helper-config -n httpbin --ignore-not-found=true
	echo -e "${COLOR_CYAN}[undeploy]${COLOR_RESET} Deleting ServiceAccount..."
	kubectl delete serviceaccount httpbin -n httpbin --ignore-not-found=true
	echo -e "${COLOR_CYAN}[undeploy]${COLOR_RESET} Deleting namespace..."
	kubectl delete namespace httpbin --ignore-not-found=true
	echo ""
	echo -e "${COLOR_BRIGHT_GREEN}[undeploy]${COLOR_RESET} ${COLOR_BOLD}httpbin removed successfully!${COLOR_RESET}"
else
	echo -e "${COLOR_YELLOW}[undeploy]${COLOR_RESET} Namespace '${COLOR_BOLD}httpbin${COLOR_RESET}' does not exist. Nothing to remove."
fi
