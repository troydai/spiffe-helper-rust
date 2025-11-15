#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo "[undeploy] Removing SPIRE server..."
if kubectl get namespace spire-server > /dev/null 2>&1; then
	echo "[undeploy] Deleting StatefulSet..."
	kubectl delete statefulset spire-server -n spire-server --ignore-not-found=true
	echo "[undeploy] Deleting Service..."
	kubectl delete service spire-server -n spire-server --ignore-not-found=true
	echo "[undeploy] Deleting ConfigMap..."
	kubectl delete configmap spire-server-config -n spire-server --ignore-not-found=true
	echo "[undeploy] Deleting Secrets..."
	kubectl delete secret spire-server-tls spire-server-ca spire-server-bootstrap -n spire-server --ignore-not-found=true
	echo "[undeploy] Deleting ServiceAccount..."
	kubectl delete serviceaccount spire-server -n spire-server --ignore-not-found=true
	echo "[undeploy] Deleting namespace..."
	kubectl delete namespace spire-server --ignore-not-found=true
	echo "[undeploy] SPIRE server removed successfully!"
else
	echo "[undeploy] Namespace 'spire-server' does not exist. Nothing to remove."
fi

