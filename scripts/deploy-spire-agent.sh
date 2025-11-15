#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"
DEPLOY_DIR="${DEPLOY_DIR:-${ROOT_DIR}/deploy/spire/agent}"
CERT_DIR="${CERT_DIR:-${ROOT_DIR}/artifacts/certs}"
BOOTSTRAP_BUNDLE="${CERT_DIR}/bootstrap-bundle.pem"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo "[deploy-spire-agent] Deploying SPIRE agent..."

if [ ! -f "${KUBECONFIG_PATH}" ]; then
	echo "Error: Kubeconfig not found. Run 'make cluster-up' first."
	exit 1
fi

if [ ! -f "${BOOTSTRAP_BUNDLE}" ]; then
	echo "Error: Bootstrap bundle not found at ${BOOTSTRAP_BUNDLE}. Run 'make certs' first."
	exit 1
fi

echo "[deploy-spire-agent] Creating namespace..."
kubectl apply -f "${DEPLOY_DIR}/namespace.yaml"

echo "[deploy-spire-agent] Creating bootstrap bundle Secret from ${BOOTSTRAP_BUNDLE}..."
kubectl create secret generic spire-bundle -n spire-agent \
	--from-file=bundle.pem="${BOOTSTRAP_BUNDLE}" \
	--dry-run=client -o yaml | \
	kubectl apply -f -

echo "[deploy-spire-agent] Applying SPIRE agent manifests..."
kubectl apply -f "${DEPLOY_DIR}/serviceaccount.yaml"
kubectl apply -f "${DEPLOY_DIR}/clusterrole.yaml"
kubectl apply -f "${DEPLOY_DIR}/clusterrolebinding.yaml"
kubectl apply -f "${DEPLOY_DIR}/configmap.yaml"
kubectl apply -f "${DEPLOY_DIR}/daemonset.yaml"

echo "[deploy-spire-agent] Waiting for SPIRE agent DaemonSet to be ready..."
timeout=300
elapsed=0
interval=5
while [ $elapsed -lt $timeout ]; do
	ready=$(kubectl get daemonset spire-agent -n spire-agent -o jsonpath='{.status.numberReady}' 2>/dev/null || echo "0")
	desired=$(kubectl get daemonset spire-agent -n spire-agent -o jsonpath='{.status.desiredNumberScheduled}' 2>/dev/null || echo "0")
	if [ "$ready" = "$desired" ] && [ "$ready" != "0" ]; then
		echo "[deploy-spire-agent] All $ready/$desired SPIRE agent pods are ready!"
		break
	fi
	echo "[deploy-spire-agent] Waiting... ($ready/$desired pods ready)"
	sleep $interval
	elapsed=$((elapsed + interval))
done

if [ $elapsed -ge $timeout ]; then
	echo "[deploy-spire-agent] Warning: Timeout waiting for DaemonSet to be ready. Checking status..."
	kubectl get daemonset spire-agent -n spire-agent
	kubectl get pods -l app=spire-agent -n spire-agent
	exit 1
fi

echo "[deploy-spire-agent] SPIRE agent DaemonSet status:"
kubectl get daemonset spire-agent -n spire-agent
echo "[deploy-spire-agent] SPIRE agent pods:"
kubectl get pods -l app=spire-agent -n spire-agent
