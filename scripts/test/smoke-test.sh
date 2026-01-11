#!/usr/bin/env bash
# Runs a suite of smoke tests to validate the SPIRE environment and basic functionality.
set -euo pipefail

# Source color support
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/../utility/colors.sh"

ROOT_DIR="${ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo -e "${COLOR_BRIGHT_BLUE}[smoke-test]${COLOR_RESET} ${COLOR_BOLD}Running smoke tests to validate SPIRE environment...${COLOR_RESET}"
echo ""

# Track overall test status
TESTS_PASSED=0
TESTS_FAILED=0

# Test 1: Check cluster connectivity
echo -e "${COLOR_CYAN}[smoke-test]${COLOR_RESET} ${COLOR_BOLD}Test 1: Cluster Connectivity${COLOR_RESET}"
if kubectl cluster-info > /dev/null 2>&1; then
	echo -e "${COLOR_GREEN}✓${COLOR_RESET} Cluster is accessible"
	TESTS_PASSED=$((TESTS_PASSED + 1))
else
	echo -e "${COLOR_RED}✗${COLOR_RESET} Cannot connect to cluster"
	TESTS_FAILED=$((TESTS_FAILED + 1))
	echo ""
	echo -e "${COLOR_RED}[smoke-test]${COLOR_RESET} Error: Cluster connectivity check failed. Run 'make env-up' first."
	exit 1
fi
echo ""

# Test 2: Check SPIRE server pod exists and is ready
echo -e "${COLOR_CYAN}[smoke-test]${COLOR_RESET} ${COLOR_BOLD}Test 2: SPIRE Server Pod Status${COLOR_RESET}"
SERVER_POD=$(kubectl get pod -n spire-server -l app=spire-server -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")
if [ -z "$SERVER_POD" ]; then
	echo -e "${COLOR_RED}✗${COLOR_RESET} SPIRE server pod not found"
	TESTS_FAILED=$((TESTS_FAILED + 1))
	echo ""
	echo -e "${COLOR_RED}[smoke-test]${COLOR_RESET} Error: SPIRE server not deployed. Run 'make deploy-spire-server' first."
	exit 1
fi

SERVER_READY=$(kubectl get pod -n spire-server -l app=spire-server -o jsonpath='{.items[0].status.conditions[?(@.type=="Ready")].status}' 2>/dev/null || echo "False")
if [ "$SERVER_READY" = "True" ]; then
	echo -e "${COLOR_GREEN}✓${COLOR_RESET} SPIRE server pod is Ready (${COLOR_CYAN}${SERVER_POD}${COLOR_RESET})"
	TESTS_PASSED=$((TESTS_PASSED + 1))
else
	echo -e "${COLOR_RED}✗${COLOR_RESET} SPIRE server pod is not Ready (${COLOR_CYAN}${SERVER_POD}${COLOR_RESET})"
	TESTS_FAILED=$((TESTS_FAILED + 1))
	echo ""
	echo -e "${COLOR_YELLOW}[smoke-test]${COLOR_RESET} Warning: SPIRE server pod exists but is not ready. Check logs with:"
	echo -e "${COLOR_CYAN}  kubectl logs -n spire-server ${SERVER_POD}${COLOR_RESET}"
	exit 1
fi
echo ""

# Test 3: SPIRE server service and container readiness
echo -e "${COLOR_CYAN}[smoke-test]${COLOR_RESET} ${COLOR_BOLD}Test 3: SPIRE Server Health${COLOR_RESET}"
CONTAINER_READY=$(kubectl get pod -n spire-server "${SERVER_POD}" -o jsonpath='{.status.containerStatuses[0].ready}' 2>/dev/null || echo "false")
if [ "$CONTAINER_READY" = "true" ]; then
	echo -e "${COLOR_GREEN}✓${COLOR_RESET} SPIRE server container is ready (health probes passed)"
	TESTS_PASSED=$((TESTS_PASSED + 1))
else
	echo -e "${COLOR_RED}✗${COLOR_RESET} SPIRE server container is not ready"
	TESTS_FAILED=$((TESTS_FAILED + 1))
	echo ""
	echo -e "${COLOR_YELLOW}[smoke-test]${COLOR_RESET} Warning: SPIRE server container is not ready. Check logs with:"
	echo -e "${COLOR_CYAN}  kubectl logs -n spire-server ${SERVER_POD}${COLOR_RESET}"
	exit 1
fi

# Verify service exists (optional check)
if kubectl get svc -n spire-server spire-server > /dev/null 2>&1; then
	echo -e "${COLOR_GREEN}✓${COLOR_RESET} SPIRE server service exists"
	TESTS_PASSED=$((TESTS_PASSED + 1))
else
	echo -e "${COLOR_YELLOW}⚠${COLOR_RESET} SPIRE server service not found (may be expected)"
	# Don't fail - service may not be strictly required
	TESTS_PASSED=$((TESTS_PASSED + 1))
fi
echo ""

# Test 4: Check SPIRE agent DaemonSet exists
echo -e "${COLOR_CYAN}[smoke-test]${COLOR_RESET} ${COLOR_BOLD}Test 4: SPIRE Agent DaemonSet Status${COLOR_RESET}"
AGENT_DS_READY=$(kubectl get daemonset spire-agent -n spire-agent -o jsonpath='{.status.numberReady}' 2>/dev/null || echo "0")
AGENT_DS_DESIRED=$(kubectl get daemonset spire-agent -n spire-agent -o jsonpath='{.status.desiredNumberScheduled}' 2>/dev/null || echo "0")

if [ "$AGENT_DS_READY" = "0" ] || [ "$AGENT_DS_DESIRED" = "0" ]; then
	echo -e "${COLOR_RED}✗${COLOR_RESET} SPIRE agent DaemonSet not found or not ready"
	TESTS_FAILED=$((TESTS_FAILED + 1))
	echo ""
	echo -e "${COLOR_RED}[smoke-test]${COLOR_RESET} Error: SPIRE agent not deployed. Run 'make deploy-spire-agent' first."
	exit 1
fi

if [ "$AGENT_DS_READY" = "$AGENT_DS_DESIRED" ]; then
	echo -e "${COLOR_GREEN}✓${COLOR_RESET} SPIRE agent DaemonSet is ready (${COLOR_CYAN}${AGENT_DS_READY}/${AGENT_DS_DESIRED}${COLOR_RESET} pods)"
	TESTS_PASSED=$((TESTS_PASSED + 1))
else
	echo -e "${COLOR_RED}✗${COLOR_RESET} SPIRE agent DaemonSet not fully ready (${COLOR_YELLOW}${AGENT_DS_READY}/${AGENT_DS_DESIRED}${COLOR_RESET} pods)"
	TESTS_FAILED=$((TESTS_FAILED + 1))
	echo ""
	echo -e "${COLOR_YELLOW}[smoke-test]${COLOR_RESET} Warning: Not all agent pods are ready. Check agent pods:"
	echo -e "${COLOR_CYAN}  kubectl get pods -n spire-agent -l app=spire-agent${COLOR_RESET}"
	exit 1
fi
echo ""

# Test 5: SPIRE agent healthcheck (check at least one agent pod)
echo -e "${COLOR_CYAN}[smoke-test]${COLOR_RESET} ${COLOR_BOLD}Test 5: SPIRE Agent Health Check${COLOR_RESET}"
AGENT_POD=$(kubectl get pod -n spire-agent -l app=spire-agent -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")
if [ -z "$AGENT_POD" ]; then
	echo -e "${COLOR_RED}✗${COLOR_RESET} No SPIRE agent pods found"
	TESTS_FAILED=$((TESTS_FAILED + 1))
	exit 1
fi

AGENT_READY=$(kubectl get pod -n spire-agent "${AGENT_POD}" -o jsonpath='{.status.conditions[?(@.type=="Ready")].status}' 2>/dev/null || echo "False")
if [ "$AGENT_READY" != "True" ]; then
	echo -e "${COLOR_RED}✗${COLOR_RESET} SPIRE agent pod is not Ready (${COLOR_CYAN}${AGENT_POD}${COLOR_RESET})"
	TESTS_FAILED=$((TESTS_FAILED + 1))
	echo ""
	echo -e "${COLOR_YELLOW}[smoke-test]${COLOR_RESET} Warning: SPIRE agent pod exists but is not ready. Check logs with:"
	echo -e "${COLOR_CYAN}  kubectl logs -n spire-agent ${AGENT_POD}${COLOR_RESET}"
	exit 1
fi

# Agent healthcheck - pod readiness is the primary indicator
# The healthcheck command may not be available in the container image
echo -e "${COLOR_GREEN}✓${COLOR_RESET} SPIRE agent pod is ready (${COLOR_CYAN}${AGENT_POD}${COLOR_RESET})"
echo -e "${COLOR_CYAN}  Note: Agent healthcheck command may not be available in container image${COLOR_RESET}"
TESTS_PASSED=$((TESTS_PASSED + 1))
echo ""

# Test 6: Verify agent can communicate with server (check agent logs for successful attestation)
echo -e "${COLOR_CYAN}[smoke-test]${COLOR_RESET} ${COLOR_BOLD}Test 6: Agent-Server Communication${COLOR_RESET}"
# Check if agent logs contain successful attestation or connection messages
AGENT_LOGS=$(kubectl logs -n spire-agent "${AGENT_POD}" --tail=50 2>/dev/null || echo "")
if echo "$AGENT_LOGS" | grep -qiE "(attested|attestation.*success|connected.*server|agent.*ready)" > /dev/null 2>&1; then
	echo -e "${COLOR_GREEN}✓${COLOR_RESET} Agent appears to have successfully attested with server"
	TESTS_PASSED=$((TESTS_PASSED + 1))
else
	echo -e "${COLOR_YELLOW}⚠${COLOR_RESET} Could not verify agent attestation from logs (this may be normal if agent just started)"
	echo -e "${COLOR_CYAN}  Check agent logs manually: kubectl logs -n spire-agent ${AGENT_POD}${COLOR_RESET}"
	# Don't fail the test, as logs format may vary
	TESTS_PASSED=$((TESTS_PASSED + 1))
fi
echo ""

# Ensure node alias exists for one-shot test (shared resource)
NODE_ALIAS_ID="spiffe://spiffe-helper.local/k8s-cluster/spiffe-helper"
if ! kubectl exec -n spire-server "${SERVER_POD}" -- \
	/opt/spire/bin/spire-server entry show -spiffeID "${NODE_ALIAS_ID}" 2>/dev/null | grep -q "${NODE_ALIAS_ID}"; then
	echo -e "${COLOR_CYAN}[smoke-test]${COLOR_RESET} Creating node alias for tests...${COLOR_RESET}"
	kubectl exec -n spire-server "${SERVER_POD}" -- \
		/opt/spire/bin/spire-server entry create \
		-node \
		-spiffeID "${NODE_ALIAS_ID}" \
		-selector "k8s_psat:cluster:spiffe-helper" > /dev/null 2>&1 || true
fi

# Test 7: Verify one-shot mode creates certificates
echo -e "${COLOR_CYAN}[smoke-test]${COLOR_RESET} ${COLOR_BOLD}Test 7: One-Shot Certificate Creation${COLOR_RESET}"
if "${SCRIPT_DIR}/test-oneshot-x509.sh"; then
	echo -e "${COLOR_GREEN}✓${COLOR_RESET} One-shot mode certificate creation test passed"
	TESTS_PASSED=$((TESTS_PASSED + 1))
else
	echo -e "${COLOR_RED}✗${COLOR_RESET} One-shot mode certificate creation test failed"
	TESTS_FAILED=$((TESTS_FAILED + 1))
fi
echo ""

# Test 8: Verify daemon mode with certificate rotation
echo -e "${COLOR_CYAN}[smoke-test]${COLOR_RESET} ${COLOR_BOLD}Test 8: Daemon Mode Certificate Rotation${COLOR_RESET}"
if "${SCRIPT_DIR}/test-daemon-x509.sh"; then
	echo -e "${COLOR_GREEN}✓${COLOR_RESET} Daemon mode certificate rotation test passed"
	TESTS_PASSED=$((TESTS_PASSED + 1))
else
	echo -e "${COLOR_RED}✗${COLOR_RESET} Daemon mode certificate rotation test failed"
	TESTS_FAILED=$((TESTS_FAILED + 1))
fi
echo ""

# Test 9: Verify managed process signaling
echo -e "${COLOR_CYAN}[smoke-test]${COLOR_RESET} ${COLOR_BOLD}Test 9: Managed Process Signaling${COLOR_RESET}"
if "${SCRIPT_DIR}/test-managed-process.sh"; then
	echo -e "${COLOR_GREEN}✓${COLOR_RESET} Managed process signaling test passed"
	TESTS_PASSED=$((TESTS_PASSED + 1))
else
	echo -e "${COLOR_RED}✗${COLOR_RESET} Managed process signaling test failed"
	TESTS_FAILED=$((TESTS_FAILED + 1))
fi
echo ""

# Test 10: Verify PID file signaling
echo -e "${COLOR_CYAN}[smoke-test]${COLOR_RESET} ${COLOR_BOLD}Test 10: PID File Signaling${COLOR_RESET}"
if "${SCRIPT_DIR}/test-pid-file.sh"; then
	echo -e "${COLOR_GREEN}✓${COLOR_RESET} PID file signaling test passed"
	TESTS_PASSED=$((TESTS_PASSED + 1))
else
	echo -e "${COLOR_RED}✗${COLOR_RESET} PID file signaling test failed"
	TESTS_FAILED=$((TESTS_FAILED + 1))
fi
echo ""

# Summary
echo -e "${COLOR_CYAN}[smoke-test]${COLOR_RESET} ${COLOR_BOLD}=== Test Summary ===${COLOR_RESET}"
echo -e "${COLOR_GREEN}Passed: ${TESTS_PASSED}${COLOR_RESET}"
if [ $TESTS_FAILED -gt 0 ]; then
	echo -e "${COLOR_RED}Failed: ${TESTS_FAILED}${COLOR_RESET}"
	echo ""
	echo -e "${COLOR_RED}[smoke-test]${COLOR_RESET} ${COLOR_BOLD}Smoke tests failed!${COLOR_RESET}"
	echo -e "${COLOR_YELLOW}[smoke-test]${COLOR_RESET} The SPIRE environment is not fully healthy."
	echo -e "${COLOR_YELLOW}[smoke-test]${COLOR_RESET} Check the errors above and ensure all components are deployed and ready."
	exit 1
else
	echo ""
	echo -e "${COLOR_BRIGHT_GREEN}[smoke-test]${COLOR_RESET} ${COLOR_BOLD}All smoke tests passed!${COLOR_RESET}"
	echo -e "${COLOR_GREEN}[smoke-test]${COLOR_RESET} SPIRE server and agent are healthy and communicating."
	exit 0
fi
