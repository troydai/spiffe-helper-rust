#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo "[undeploy-spire-agent] Removing SPIRE agent..."

if [ ! -f "${KUBECONFIG_PATH}" ]; then
	echo "Kubeconfig not found. Skipping undeploy."
	exit 0
fi

kubectl delete daemonset spire-agent -n spire-agent --ignore-not-found=true
kubectl delete configmap spire-agent-config -n spire-agent --ignore-not-found=true
kubectl delete clusterrolebinding spire-agent-cluster-role-binding --ignore-not-found=true
kubectl delete clusterrole spire-agent-cluster-role --ignore-not-found=true
kubectl delete serviceaccount spire-agent -n spire-agent --ignore-not-found=true
kubectl delete secret spire-bundle -n spire-agent --ignore-not-found=true
kubectl delete namespace spire-agent --ignore-not-found=true

echo "[undeploy-spire-agent] SPIRE agent removed."
