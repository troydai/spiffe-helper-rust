#!/usr/bin/env bash
set -euo pipefail

# Source color support
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/colors.sh"

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo -e "${COLOR_BRIGHT_BLUE}[check-spire-server]${COLOR_RESET} ${COLOR_BOLD}Checking SPIRE server status...${COLOR_RESET}"
echo ""

echo -e "${COLOR_CYAN}[check-spire-server]${COLOR_RESET} ${COLOR_BOLD}=== Pod Status ===${COLOR_RESET}"
if ! kubectl get pods -n spire-server -l app=spire-server; then
	echo -e "${COLOR_RED}[check-spire-server] Error:${COLOR_RESET} SPIRE server namespace or pods not found. Run 'make deploy-spire-server' first."
	exit 1
fi

echo ""
echo -e "${COLOR_CYAN}[check-spire-server]${COLOR_RESET} ${COLOR_BOLD}=== Service Status ===${COLOR_RESET}"
kubectl get svc -n spire-server spire-server || echo -e "${COLOR_YELLOW}[check-spire-server]${COLOR_RESET} Service not found"

echo ""
echo -e "${COLOR_CYAN}[check-spire-server]${COLOR_RESET} ${COLOR_BOLD}=== Pod Logs (last 20 lines) ===${COLOR_RESET}"
kubectl logs -n spire-server -l app=spire-server --tail=20 || echo -e "${COLOR_YELLOW}[check-spire-server]${COLOR_RESET} Unable to fetch logs"

echo ""
echo -e "${COLOR_CYAN}[check-spire-server]${COLOR_RESET} ${COLOR_BOLD}=== Health Check ===${COLOR_RESET}"
if kubectl get pod -n spire-server -l app=spire-server -o jsonpath='{.items[0].status.conditions[?(@.type=="Ready")].status}' | grep -q "True"; then
	echo -e "${COLOR_GREEN}✓${COLOR_RESET} SPIRE server pod is Ready"
else
	echo -e "${COLOR_RED}✗${COLOR_RESET} SPIRE server pod is not Ready"
fi

if kubectl get pod -n spire-server -l app=spire-server -o jsonpath='{.items[0].status.containerStatuses[0].ready}' | grep -q "true"; then
	echo -e "${COLOR_GREEN}✓${COLOR_RESET} SPIRE server container is ready"
else
	echo -e "${COLOR_RED}✗${COLOR_RESET} SPIRE server container is not ready"
fi

echo ""
echo -e "${COLOR_CYAN}[check-spire-server]${COLOR_RESET} To view full logs: ${COLOR_BOLD}kubectl logs -n spire-server -l app=spire-server -f${COLOR_RESET}"
echo -e "${COLOR_CYAN}[check-spire-server]${COLOR_RESET} To exec into pod: ${COLOR_BOLD}kubectl exec -it -n spire-server -l app=spire-server -- /bin/sh${COLOR_RESET}"

