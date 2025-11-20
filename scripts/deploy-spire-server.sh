#!/usr/bin/env bash
set -euo pipefail

# Source color support
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/colors.sh"

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"
DEPLOY_DIR="${DEPLOY_DIR:-${ROOT_DIR}/deploy/spire/server}"
CERT_DIR="${CERT_DIR:-${ROOT_DIR}/artifacts/certs}"
KIND="${KIND:-kind}"
KIND_CLUSTER_NAME="${KIND_CLUSTER_NAME:-spiffe-helper}"

export KUBECONFIG="${KUBECONFIG_PATH}"

# Certificate file paths
CA_CERT="${CERT_DIR}/ca-cert.pem"
CA_KEY="${CERT_DIR}/ca-key.pem"
SERVER_CERT="${CERT_DIR}/spire-server-cert.pem"
SERVER_KEY="${CERT_DIR}/spire-server-key.pem"
BOOTSTRAP_BUNDLE="${CERT_DIR}/bootstrap-bundle.pem"

echo -e "${COLOR_BRIGHT_BLUE}[deploy]${COLOR_RESET} ${COLOR_BOLD}Deploying SPIRE server...${COLOR_RESET}"
echo -e "${COLOR_CYAN}[deploy]${COLOR_RESET} Creating namespace..."
kubectl apply -f "${DEPLOY_DIR}/namespace.yaml"

echo -e "${COLOR_CYAN}[deploy]${COLOR_RESET} Creating ServiceAccount..."
kubectl apply -f "${DEPLOY_DIR}/serviceaccount.yaml"
echo -e "${COLOR_CYAN}[deploy]${COLOR_RESET} Creating ClusterRole and ClusterRoleBinding..."
kubectl apply -f "${DEPLOY_DIR}/clusterrole.yaml"
kubectl apply -f "${DEPLOY_DIR}/clusterrolebinding.yaml"

echo -e "${COLOR_CYAN}[deploy]${COLOR_RESET} Creating Secrets from certificates..."
kubectl create secret generic spire-server-tls \
	--from-file=server.crt="${SERVER_CERT}" \
	--from-file=server.key="${SERVER_KEY}" \
	--namespace=spire-server \
	--dry-run=client -o yaml | kubectl apply -f -

kubectl create secret generic spire-server-ca \
	--from-file=ca.crt="${CA_CERT}" \
	--from-file=ca.key="${CA_KEY}" \
	--namespace=spire-server \
	--dry-run=client -o yaml | kubectl apply -f -

kubectl create secret generic spire-server-bootstrap \
	--from-file=bundle.pem="${BOOTSTRAP_BUNDLE}" \
	--namespace=spire-server \
	--dry-run=client -o yaml | kubectl apply -f -

echo -e "${COLOR_CYAN}[deploy]${COLOR_RESET} Creating ConfigMap..."
kubectl apply -f "${DEPLOY_DIR}/configmap.yaml"

echo -e "${COLOR_CYAN}[deploy]${COLOR_RESET} Creating Service..."
kubectl apply -f "${DEPLOY_DIR}/service.yaml"

echo -e "${COLOR_CYAN}[deploy]${COLOR_RESET} Creating Deployment..."
kubectl apply -f "${DEPLOY_DIR}/deployment.yaml"

echo -e "${COLOR_CYAN}[deploy]${COLOR_RESET} Waiting for Deployment rollout..."
kubectl rollout status deployment/spire-server -n spire-server --timeout=300s

echo -e "${COLOR_CYAN}[deploy]${COLOR_RESET} Waiting for pod to be ready..."
if ! kubectl wait --for=condition=ready pod -l app=spire-server -n spire-server --timeout=300s; then
	echo -e "${COLOR_YELLOW}[deploy]${COLOR_RESET} Warning: Pod may not be fully ready. Check with: kubectl get pods -n spire-server"
	exit 1
fi

echo ""
echo -e "${COLOR_BRIGHT_GREEN}[deploy]${COLOR_RESET} ${COLOR_BOLD}SPIRE server deployed successfully!${COLOR_RESET}"
echo -e "${COLOR_CYAN}[deploy]${COLOR_RESET} Pod status:"
kubectl get pods -n spire-server
