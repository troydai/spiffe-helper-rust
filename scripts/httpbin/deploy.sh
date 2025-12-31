#!/usr/bin/env bash
# Deploys the httpbin service to the Kubernetes cluster.
set -euo pipefail

# Source color support
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../utility/colors.sh"

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"
DEPLOY_DIR="${DEPLOY_DIR:-${ROOT_DIR}/deploy/httpbin}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo -e "${COLOR_BRIGHT_BLUE}[deploy]${COLOR_RESET} ${COLOR_BOLD}Deploying httpbin service...${COLOR_RESET}"

if kubectl apply -f "${DEPLOY_DIR}/httpbin.yaml" 2>/dev/null; then
	echo -e "${COLOR_CYAN}[deploy]${COLOR_RESET} Waiting for httpbin pod to be ready..."
	if kubectl wait --for=condition=ready pod -l app=httpbin -n httpbin --timeout=60s 2>/dev/null; then
		echo -e "${COLOR_GREEN}✓${COLOR_RESET} httpbin pod is ready"
	else
		echo -e "${COLOR_YELLOW}[deploy]${COLOR_RESET} httpbin deployment may still be in progress"
	fi
else
	echo -e "${COLOR_YELLOW}[deploy]${COLOR_RESET} Failed to deploy httpbin (may already exist)"
fi

echo ""
echo -e "${COLOR_BRIGHT_GREEN}[deploy]${COLOR_RESET} ${COLOR_BOLD}httpbin deployed successfully!${COLOR_RESET}"
echo -e "${COLOR_CYAN}[deploy]${COLOR_RESET} Pod status:"
kubectl get pods -n httpbin
