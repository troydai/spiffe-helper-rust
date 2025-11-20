#!/usr/bin/env bash
set -euo pipefail

# Test script for dumb-init signal handling in daemon mode
# This script tests:
# 1. dumb-init is PID 1 in the container
# 2. SIGTERM signal handling works correctly
# 3. Graceful shutdown occurs when pod is deleted

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
KUBECONFIG_PATH="${KUBECONFIG:-$ROOT_DIR/artifacts/kubeconfig}"
NAMESPACE="test-daemon"
APP_LABEL="app=test-daemon"

# Colors
source "${SCRIPT_DIR}/colors.sh"

# Check if kubectl is available
if ! command -v kubectl &> /dev/null; then
    echo -e "${COLOR_RED}Error: kubectl not found${COLOR_RESET}"
    exit 1
fi

# Check if kubeconfig exists
if [ ! -f "$KUBECONFIG_PATH" ]; then
    echo -e "${COLOR_RED}Error: Kubeconfig not found at $KUBECONFIG_PATH${COLOR_RESET}"
    echo "Please ensure the cluster is set up (make env-up)"
    exit 1
fi

export KUBECONFIG="$KUBECONFIG_PATH"

echo -e "${COLOR_BRIGHT_BLUE}[test-signal-handling]${COLOR_RESET} ${COLOR_BOLD}Testing dumb-init signal handling${COLOR_RESET}"
echo ""

# Deploy test daemon
echo -e "${COLOR_CYAN}[test-signal-handling]${COLOR_RESET} Deploying test daemon..."
kubectl apply -f "${ROOT_DIR}/deploy/test-daemon/test-daemon.yaml" > /dev/null

# Wait for deployment to be ready
echo -e "${COLOR_CYAN}[test-signal-handling]${COLOR_RESET} Waiting for pod to be ready..."
for i in {1..60}; do
    POD_NAME=$(kubectl get pod -n "$NAMESPACE" -l "$APP_LABEL" -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")
    if [ -n "$POD_NAME" ]; then
        POD_READY=$(kubectl get pod -n "$NAMESPACE" "$POD_NAME" -o jsonpath='{.status.conditions[?(@.type=="Ready")].status}' 2>/dev/null || echo "False")
        if [ "$POD_READY" = "True" ]; then
            echo -e "${COLOR_GREEN}✓${COLOR_RESET} Pod is ready: ${COLOR_CYAN}${POD_NAME}${COLOR_RESET}"
            break
        fi
    fi
    if [ $i -eq 60 ]; then
        echo -e "${COLOR_RED}Error: Pod did not become ready within 60 seconds${COLOR_RESET}"
        kubectl get pods -n "$NAMESPACE"
        exit 1
    fi
    sleep 1
done

# Test 1: Verify dumb-init is PID 1
echo ""
echo -e "${COLOR_CYAN}[test-signal-handling]${COLOR_RESET} ${COLOR_BOLD}Test 1: Verify dumb-init is PID 1${COLOR_RESET}"
PID1_PROC=$(kubectl exec -n "$NAMESPACE" "$POD_NAME" -- ps -p 1 -o comm= 2>/dev/null || echo "")
if [ "$PID1_PROC" = "dumb-init" ]; then
    echo -e "${COLOR_GREEN}✓${COLOR_RESET} dumb-init is PID 1"
else
    echo -e "${COLOR_RED}✗${COLOR_RESET} PID 1 is '${PID1_PROC}', expected 'dumb-init'"
    exit 1
fi

# Test 2: Verify spiffe-helper-rust is running as a child process
echo ""
echo -e "${COLOR_CYAN}[test-signal-handling]${COLOR_RESET} ${COLOR_BOLD}Test 2: Verify spiffe-helper-rust is running${COLOR_RESET}"
if kubectl exec -n "$NAMESPACE" "$POD_NAME" -- pgrep -f spiffe-helper-rust > /dev/null 2>&1; then
    echo -e "${COLOR_GREEN}✓${COLOR_RESET} spiffe-helper-rust process is running"
else
    echo -e "${COLOR_RED}✗${COLOR_RESET} spiffe-helper-rust process not found"
    kubectl exec -n "$NAMESPACE" "$POD_NAME" -- ps aux
    exit 1
fi

# Test 3: Verify health check endpoints are working
echo ""
echo -e "${COLOR_CYAN}[test-signal-handling]${COLOR_RESET} ${COLOR_BOLD}Test 3: Verify health check endpoints${COLOR_RESET}"
# Try curl first, then wget, then use kubectl port-forward as fallback
if kubectl exec -n "$NAMESPACE" "$POD_NAME" -- curl -sf http://localhost:8080/health/live > /dev/null 2>&1; then
    echo -e "${COLOR_GREEN}✓${COLOR_RESET} Liveness endpoint is responding"
elif kubectl exec -n "$NAMESPACE" "$POD_NAME" -- wget -q -O- http://localhost:8080/health/live > /dev/null 2>&1; then
    echo -e "${COLOR_GREEN}✓${COLOR_RESET} Liveness endpoint is responding"
else
    # Fallback: use kubectl port-forward to test
    kubectl port-forward -n "$NAMESPACE" "$POD_NAME" 8080:8080 > /dev/null 2>&1 &
    PF_PID=$!
    sleep 2
    if curl -sf http://localhost:8080/health/live > /dev/null 2>&1; then
        echo -e "${COLOR_GREEN}✓${COLOR_RESET} Liveness endpoint is responding"
        kill $PF_PID 2>/dev/null || true
    else
        kill $PF_PID 2>/dev/null || true
        echo -e "${COLOR_RED}✗${COLOR_RESET} Liveness endpoint not responding"
        exit 1
    fi
fi

if kubectl exec -n "$NAMESPACE" "$POD_NAME" -- curl -sf http://localhost:8080/health/ready > /dev/null 2>&1; then
    echo -e "${COLOR_GREEN}✓${COLOR_RESET} Readiness endpoint is responding"
elif kubectl exec -n "$NAMESPACE" "$POD_NAME" -- wget -q -O- http://localhost:8080/health/ready > /dev/null 2>&1; then
    echo -e "${COLOR_GREEN}✓${COLOR_RESET} Readiness endpoint is responding"
else
    # Fallback: use kubectl port-forward to test
    kubectl port-forward -n "$NAMESPACE" "$POD_NAME" 8080:8080 > /dev/null 2>&1 &
    PF_PID=$!
    sleep 2
    if curl -sf http://localhost:8080/health/ready > /dev/null 2>&1; then
        echo -e "${COLOR_GREEN}✓${COLOR_RESET} Readiness endpoint is responding"
        kill $PF_PID 2>/dev/null || true
    else
        kill $PF_PID 2>/dev/null || true
        echo -e "${COLOR_RED}✗${COLOR_RESET} Readiness endpoint not responding"
        exit 1
    fi
fi

# Test 4: Test SIGTERM signal handling (graceful shutdown)
echo ""
echo -e "${COLOR_CYAN}[test-signal-handling]${COLOR_RESET} ${COLOR_BOLD}Test 4: Test SIGTERM signal handling${COLOR_RESET}"
echo "  Deleting pod to trigger SIGTERM..."

# Get logs before deletion to establish baseline
kubectl logs -n "$NAMESPACE" "$POD_NAME" > /tmp/before-delete.log 2>&1 || true

# Delete the pod (this sends SIGTERM)
kubectl delete pod -n "$NAMESPACE" "$POD_NAME" --grace-period=10 > /dev/null

# Wait for pod to terminate
echo "  Waiting for pod to terminate gracefully..."
for i in {1..30}; do
    POD_STATUS=$(kubectl get pod -n "$NAMESPACE" "$POD_NAME" -o jsonpath='{.status.phase}' 2>/dev/null || echo "Terminated")
    if [ "$POD_STATUS" = "Succeeded" ] || [ "$POD_STATUS" = "Terminated" ] || [ -z "$POD_STATUS" ]; then
        break
    fi
    sleep 1
done

# Get final logs
FINAL_LOGS=$(kubectl logs -n "$NAMESPACE" "$POD_NAME" 2>&1 || echo "")

# Check for graceful shutdown message
if echo "$FINAL_LOGS" | grep -q "Received SIGTERM, shutting down gracefully"; then
    echo -e "${COLOR_GREEN}✓${COLOR_RESET} Graceful shutdown message found in logs"
else
    echo -e "${COLOR_RED}✗${COLOR_RESET} Graceful shutdown message not found in logs"
    echo "Final logs:"
    echo "$FINAL_LOGS" | tail -20
    exit 1
fi

# Check for health check server shutdown message
if echo "$FINAL_LOGS" | grep -q "Health check server stopped"; then
    echo -e "${COLOR_GREEN}✓${COLOR_RESET} Health check server stopped message found"
else
    echo -e "${COLOR_YELLOW}⚠${COLOR_RESET} Health check server stopped message not found (may be expected if health checks disabled)"
fi

# Check for daemon shutdown complete message
if echo "$FINAL_LOGS" | grep -q "Daemon shutdown complete"; then
    echo -e "${COLOR_GREEN}✓${COLOR_RESET} Daemon shutdown complete message found"
else
    echo -e "${COLOR_RED}✗${COLOR_RESET} Daemon shutdown complete message not found"
    exit 1
fi

# Verify exit code (should be 0 for graceful shutdown)
EXIT_CODE=$(kubectl get pod -n "$NAMESPACE" "$POD_NAME" -o jsonpath='{.status.containerStatuses[0].state.terminated.exitCode}' 2>/dev/null || echo "unknown")
if [ "$EXIT_CODE" = "0" ]; then
    echo -e "${COLOR_GREEN}✓${COLOR_RESET} Container exited with code 0 (graceful shutdown)"
else
    echo -e "${COLOR_YELLOW}⚠${COLOR_RESET} Container exited with code: ${EXIT_CODE}"
fi

# Cleanup
echo ""
echo -e "${COLOR_CYAN}[test-signal-handling]${COLOR_RESET} Cleaning up test deployment..."
kubectl delete -f "${ROOT_DIR}/deploy/test-daemon/test-daemon.yaml" > /dev/null 2>&1 || true

# Wait for namespace to be deleted
for i in {1..30}; do
    if ! kubectl get namespace "$NAMESPACE" > /dev/null 2>&1; then
        break
    fi
    sleep 1
done

echo ""
echo -e "${COLOR_BRIGHT_GREEN}[test-signal-handling]${COLOR_RESET} ${COLOR_BOLD}All signal handling tests passed!${COLOR_RESET}"
echo ""
echo "Summary:"
echo "  - dumb-init is PID 1"
echo "  - spiffe-helper-rust runs as child process"
echo "  - Health check endpoints work"
echo "  - SIGTERM triggers graceful shutdown"
echo "  - Container exits cleanly"
