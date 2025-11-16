#!/usr/bin/env bash
set -euo pipefail

# Source color support
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/colors.sh"

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
KIND="${KIND:-kind}"
KIND_CLUSTER_NAME="${KIND_CLUSTER_NAME:-spiffe-helper}"
KIND_CONFIG="${KIND_CONFIG:-${ROOT_DIR}/kind-config.yaml}"
ARTIFACTS_DIR="${ARTIFACTS_DIR:-${ROOT_DIR}/artifacts}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ARTIFACTS_DIR}/kubeconfig}"

# Create artifacts directory
mkdir -p "${ARTIFACTS_DIR}"

echo -e "${COLOR_BRIGHT_BLUE}[cluster-up]${COLOR_RESET} ${COLOR_BOLD}Setting up kind cluster...${COLOR_RESET}"

# Check if cluster already exists
if ${KIND} get clusters | grep -qx "${KIND_CLUSTER_NAME}"; then
	echo -e "${COLOR_YELLOW}[cluster-up]${COLOR_RESET} kind cluster '${COLOR_BOLD}${KIND_CLUSTER_NAME}${COLOR_RESET}' already exists"
else
	echo -e "${COLOR_CYAN}[cluster-up]${COLOR_RESET} Creating kind cluster '${COLOR_BOLD}${KIND_CLUSTER_NAME}${COLOR_RESET}'..."
	KUBECONFIG="${KUBECONFIG_PATH}" ${KIND} create cluster --name "${KIND_CLUSTER_NAME}" --config "${KIND_CONFIG}"
fi

# Get kubeconfig
echo -e "${COLOR_CYAN}[cluster-up]${COLOR_RESET} Writing kubeconfig..."
${KIND} get kubeconfig --name "${KIND_CLUSTER_NAME}" > "${KUBECONFIG_PATH}"
echo -e "${COLOR_GREEN}✓${COLOR_RESET} Kubeconfig written to ${COLOR_CYAN}${KUBECONFIG_PATH}${COLOR_RESET}"

# Deploy httpbin service
echo -e "${COLOR_CYAN}[cluster-up]${COLOR_RESET} Deploying httpbin service..."
export KUBECONFIG="${KUBECONFIG_PATH}"
if kubectl apply -f "${ROOT_DIR}/deploy/httpbin/httpbin.yaml" 2>/dev/null; then
	echo -e "${COLOR_CYAN}[cluster-up]${COLOR_RESET} Waiting for httpbin pod to be ready..."
	if kubectl wait --for=condition=ready pod -l app=httpbin -n httpbin --timeout=60s 2>/dev/null; then
		echo -e "${COLOR_GREEN}✓${COLOR_RESET} httpbin pod is ready"
	else
		echo -e "${COLOR_YELLOW}[cluster-up]${COLOR_RESET} httpbin deployment may still be in progress"
	fi
else
	echo -e "${COLOR_YELLOW}[cluster-up]${COLOR_RESET} Failed to deploy httpbin (may already exist)"
fi

echo ""
echo -e "${COLOR_BRIGHT_GREEN}[cluster-up]${COLOR_RESET} ${COLOR_BOLD}Cluster setup complete!${COLOR_RESET}"
