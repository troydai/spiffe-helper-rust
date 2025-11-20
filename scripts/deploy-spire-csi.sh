#!/usr/bin/env bash
set -euo pipefail

# Source color support
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/colors.sh"

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"
DEPLOY_DIR="${DEPLOY_DIR:-${ROOT_DIR}/deploy/spire/csi}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo -e "${COLOR_BRIGHT_BLUE}[deploy-spire-csi]${COLOR_RESET} ${COLOR_BOLD}Deploying SPIRE CSI driver...${COLOR_RESET}"

if [ ! -f "${KUBECONFIG_PATH}" ]; then
	echo -e "${COLOR_RED}[deploy-spire-csi] Error:${COLOR_RESET} Kubeconfig not found. Run 'make cluster-up' first."
	exit 1
fi

echo -e "${COLOR_CYAN}[deploy-spire-csi]${COLOR_RESET} Creating namespace..."
kubectl apply -f "${DEPLOY_DIR}/namespace.yaml"

echo -e "${COLOR_CYAN}[deploy-spire-csi]${COLOR_RESET} Applying SPIRE CSI driver manifests..."
kubectl apply -f "${DEPLOY_DIR}/serviceaccount.yaml"
kubectl apply -f "${DEPLOY_DIR}/clusterrole.yaml"
kubectl apply -f "${DEPLOY_DIR}/clusterrolebinding.yaml"
kubectl apply -f "${DEPLOY_DIR}/csidriver.yaml"
kubectl apply -f "${DEPLOY_DIR}/configmap.yaml"
kubectl apply -f "${DEPLOY_DIR}/daemonset.yaml"

echo -e "${COLOR_CYAN}[deploy-spire-csi]${COLOR_RESET} Waiting for SPIRE CSI driver DaemonSet to be ready..."
timeout=300
elapsed=0
interval=5
while [ $elapsed -lt $timeout ]; do
	ready=$(kubectl get daemonset spire-csi-driver -n spire-csi -o jsonpath='{.status.numberReady}' 2>/dev/null || echo "0")
	desired=$(kubectl get daemonset spire-csi-driver -n spire-csi -o jsonpath='{.status.desiredNumberScheduled}' 2>/dev/null || echo "0")
	
	if [ "$ready" = "$desired" ] && [ "$desired" != "0" ]; then
		echo -e "${COLOR_GREEN}✓${COLOR_RESET} SPIRE CSI driver DaemonSet is ready (${ready}/${desired} pods ready)"
		break
	fi
	
	echo -e "${COLOR_CYAN}[deploy-spire-csi]${COLOR_RESET} Waiting for pods to be ready... (${ready}/${desired} ready, ${elapsed}s elapsed)"
	sleep $interval
	elapsed=$((elapsed + interval))
done

if [ $elapsed -ge $timeout ]; then
	echo -e "${COLOR_YELLOW}[deploy-spire-csi]${COLOR_RESET} Warning: Timeout waiting for DaemonSet to be ready. Check pod status:"
	kubectl get pods -n spire-csi
	exit 1
fi

echo ""
echo -e "${COLOR_BRIGHT_GREEN}[deploy-spire-csi]${COLOR_RESET} ${COLOR_BOLD}SPIRE CSI driver deployed successfully!${COLOR_RESET}"
echo -e "${COLOR_CYAN}[deploy-spire-csi]${COLOR_RESET} Pod status:"
kubectl get pods -n spire-csi
