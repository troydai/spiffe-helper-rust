#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo "[check] Checking SPIRE server status..."
echo ""
echo "=== Pod Status ==="
if ! kubectl get pods -n spire-server -l app=spire-server; then
	echo "Error: SPIRE server namespace or pods not found. Run 'make deploy-spire-server' first."
	exit 1
fi

echo ""
echo "=== Service Status ==="
kubectl get svc -n spire-server spire-server || echo "Service not found"

echo ""
echo "=== Pod Logs (last 20 lines) ==="
kubectl logs -n spire-server -l app=spire-server --tail=20 || echo "Unable to fetch logs"

echo ""
echo "=== Health Check ==="
if kubectl get pod -n spire-server -l app=spire-server -o jsonpath='{.items[0].status.conditions[?(@.type=="Ready")].status}' | grep -q "True"; then
	echo "✓ SPIRE server pod is Ready"
else
	echo "✗ SPIRE server pod is not Ready"
fi

if kubectl get pod -n spire-server -l app=spire-server -o jsonpath='{.items[0].status.containerStatuses[0].ready}' | grep -q "true"; then
	echo "✓ SPIRE server container is ready"
else
	echo "✗ SPIRE server container is not ready"
fi

echo ""
echo "To view full logs: kubectl logs -n spire-server -l app=spire-server -f"
echo "To exec into pod: kubectl exec -it -n spire-server -l app=spire-server -- /bin/sh"

