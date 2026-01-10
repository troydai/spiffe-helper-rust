#!/usr/bin/env bash
# Deploys the SPIRE agent DaemonSet to the Kubernetes cluster.
set -euo pipefail

# Source color support
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../utility/colors.sh"

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"
DEPLOY_DIR="${DEPLOY_DIR:-${ROOT_DIR}/deploy/spire/agent}"
CERT_DIR="${CERT_DIR:-${ROOT_DIR}/artifacts/certs}"
BOOTSTRAP_BUNDLE="${CERT_DIR}/bootstrap-bundle.pem"
KIND="${KIND:-kind}"
KIND_CLUSTER_NAME="${KIND_CLUSTER_NAME:-spiffe-helper}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo -e "${COLOR_BRIGHT_BLUE}[deploy-spire-agent]${COLOR_RESET} ${COLOR_BOLD}Deploying SPIRE agent...${COLOR_RESET}"

if [ ! -f "${KUBECONFIG_PATH}" ]; then
	echo -e "${COLOR_RED}[deploy-spire-agent] Error:${COLOR_RESET} Kubeconfig not found. Run 'make cluster-up' first."
	exit 1
fi

if [ ! -f "${BOOTSTRAP_BUNDLE}" ]; then
	echo -e "${COLOR_RED}[deploy-spire-agent] Error:${COLOR_RESET} Bootstrap bundle not found at ${COLOR_CYAN}${BOOTSTRAP_BUNDLE}${COLOR_RESET}. Run 'make certs' first."
	exit 1
fi

echo -e "${COLOR_CYAN}[deploy-spire-agent]${COLOR_RESET} Creating namespace..."
kubectl apply -f "${DEPLOY_DIR}/namespace.yaml"

echo -e "${COLOR_CYAN}[deploy-spire-agent]${COLOR_RESET} Creating bootstrap bundle Secret from ${COLOR_CYAN}${BOOTSTRAP_BUNDLE}${COLOR_RESET}..."
kubectl create secret generic spire-bundle -n spire-agent \
	--from-file=bundle.pem="${BOOTSTRAP_BUNDLE}" \
	--dry-run=client -o yaml | \
	kubectl apply -f -

echo -e "${COLOR_CYAN}[deploy-spire-agent]${COLOR_RESET} Applying SPIRE agent manifests..."
kubectl apply -f "${DEPLOY_DIR}/serviceaccount.yaml"
kubectl apply -f "${DEPLOY_DIR}/clusterrole.yaml"
kubectl apply -f "${DEPLOY_DIR}/clusterrolebinding.yaml"
kubectl apply -f "${DEPLOY_DIR}/configmap.yaml"
kubectl apply -f "${DEPLOY_DIR}/daemonset.yaml"

echo -e "${COLOR_CYAN}[deploy-spire-agent]${COLOR_RESET} Waiting for SPIRE agent DaemonSet to be ready..."
timeout=300
elapsed=0
interval=5
while [ $elapsed -lt $timeout ]; do
	ready=$(kubectl get daemonset spire-agent -n spire-agent -o jsonpath='{.status.numberReady}' 2>/dev/null || echo "0")
	desired=$(kubectl get daemonset spire-agent -n spire-agent -o jsonpath='{.status.desiredNumberScheduled}' 2>/dev/null || echo "0")
	if [ "$ready" = "$desired" ] && [ "$ready" != "0" ]; then
		echo -e "${COLOR_GREEN}[deploy-spire-agent]${COLOR_RESET} All ${COLOR_BOLD}${ready}/${desired}${COLOR_RESET} SPIRE agent pods are ready!"
		break
	fi
	echo -e "${COLOR_CYAN}[deploy-spire-agent]${COLOR_RESET} Waiting... (${COLOR_YELLOW}${ready}/${desired}${COLOR_RESET} pods ready)"
	sleep $interval
	elapsed=$((elapsed + interval))
done

if [ $elapsed -ge $timeout ]; then
	echo -e "${COLOR_YELLOW}[deploy-spire-agent]${COLOR_RESET} Warning: Timeout waiting for DaemonSet to be ready. Checking status..."
	kubectl get daemonset spire-agent -n spire-agent
	kubectl get pods -l app=spire-agent -n spire-agent
	exit 1
fi

echo ""
echo -e "${COLOR_BRIGHT_GREEN}[deploy-spire-agent]${COLOR_RESET} ${COLOR_BOLD}SPIRE agent deployed successfully!${COLOR_RESET}"
echo -e "${COLOR_CYAN}[deploy-spire-agent]${COLOR_RESET} SPIRE agent DaemonSet status:"
kubectl get daemonset spire-agent -n spire-agent
echo -e "${COLOR_CYAN}[deploy-spire-agent]${COLOR_RESET} SPIRE agent pods:"
kubectl get pods -l app=spire-agent -n spire-agent
