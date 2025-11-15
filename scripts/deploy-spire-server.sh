#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"
DEPLOY_DIR="${DEPLOY_DIR:-${ROOT_DIR}/deploy/spire/server}"
CERT_DIR="${CERT_DIR:-${ROOT_DIR}/artifacts/certs}"

export KUBECONFIG="${KUBECONFIG_PATH}"

# Certificate file paths
CA_CERT="${CERT_DIR}/ca-cert.pem"
CA_KEY="${CERT_DIR}/ca-key.pem"
SERVER_CERT="${CERT_DIR}/spire-server-cert.pem"
SERVER_KEY="${CERT_DIR}/spire-server-key.pem"
BOOTSTRAP_BUNDLE="${CERT_DIR}/bootstrap-bundle.pem"

echo "[deploy] Deploying SPIRE server..."
echo "[deploy] Creating namespace..."
kubectl apply -f "${DEPLOY_DIR}/namespace.yaml"

echo "[deploy] Creating ServiceAccount..."
kubectl apply -f "${DEPLOY_DIR}/serviceaccount.yaml"

echo "[deploy] Creating Secrets from certificates..."
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

echo "[deploy] Creating ConfigMap..."
kubectl apply -f "${DEPLOY_DIR}/configmap.yaml"

echo "[deploy] Creating Service..."
kubectl apply -f "${DEPLOY_DIR}/service.yaml"

echo "[deploy] Creating StatefulSet..."
kubectl apply -f "${DEPLOY_DIR}/statefulset.yaml"

echo "[deploy] Waiting for StatefulSet rollout..."
kubectl rollout status statefulset/spire-server -n spire-server --timeout=300s

echo "[deploy] Waiting for pod to be ready..."
if ! kubectl wait --for=condition=ready pod -l app=spire-server -n spire-server --timeout=300s; then
	echo "[deploy] Warning: Pod may not be fully ready. Check with: kubectl get pods -n spire-server"
	exit 1
fi

echo "[deploy] SPIRE server deployed successfully!"
echo "[deploy] Pod status:"
kubectl get pods -n spire-server
