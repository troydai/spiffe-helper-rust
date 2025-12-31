#!/bin/bash
# Deploys the SPIRE CSI driver to the Kubernetes cluster.

set -e

# Source color support
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../utility/colors.sh"

ROOT_DIR="${ROOT_DIR:-$(cd "${SCRIPT_DIR}/../.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo -e "${COLOR_BRIGHT_BLUE}[deploy-csi]${COLOR_RESET} ${COLOR_BOLD}Deploying SPIRE CSI driver...${COLOR_RESET}"
kubectl apply -f "${SCRIPT_DIR}/../../deploy/spire/csi/"
echo -e "${COLOR_GREEN}✓${COLOR_RESET} SPIRE CSI driver deployed"
