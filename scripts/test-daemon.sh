#!/bin/bash
set -e

# Test script for spiffe-helper functionality
# This script tests initContainer behavior through httpbin pods:
# 1. InitContainer starts and completes successfully
# 2. Main container starts after initContainer
# 3. Pod lifecycle (delete/recreate) works correctly

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
KUBECONFIG_PATH="${KUBECONFIG:-$ROOT_DIR/artifacts/kubeconfig}"
NAMESPACE="httpbin"
APP_LABEL="app=httpbin"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if kubectl is available
if ! command -v kubectl &> /dev/null; then
    echo -e "${RED}Error: kubectl not found${NC}"
    exit 1
fi

# Check if kubeconfig exists
if [ ! -f "$KUBECONFIG_PATH" ]; then
    echo -e "${RED}Error: Kubeconfig not found at $KUBECONFIG_PATH${NC}"
    echo "Please ensure the cluster is set up (make env-up)"
    exit 1
fi

export KUBECONFIG="$KUBECONFIG_PATH"

echo -e "${GREEN}=== Testing spiffe-helper initContainer via httpbin ===${NC}"
echo ""

# Check if httpbin namespace exists
if ! kubectl get namespace "$NAMESPACE" &> /dev/null; then
    echo -e "${RED}Error: Namespace $NAMESPACE not found${NC}"
    echo "Please deploy httpbin first: kubectl apply -f deploy/httpbin/httpbin.yaml"
    exit 1
fi

# Check if httpbin pods exist
if ! kubectl get pods -n "$NAMESPACE" -l "$APP_LABEL" &> /dev/null; then
    echo -e "${RED}Error: No httpbin pods found${NC}"
    echo "Please deploy httpbin first: kubectl apply -f deploy/httpbin/httpbin.yaml"
    exit 1
fi

echo -e "${GREEN}[1/3] Checking initial pod status...${NC}"
INITIAL_POD=$(kubectl get pods -n "$NAMESPACE" -l "$APP_LABEL" -o jsonpath='{.items[0].metadata.name}' 2>/dev/null)
if [ -z "$INITIAL_POD" ]; then
    echo -e "${RED}Error: Could not find httpbin pod${NC}"
    exit 1
fi

echo "  Found pod: $INITIAL_POD"
INIT_STATUS=$(kubectl get pod -n "$NAMESPACE" "$INITIAL_POD" -o jsonpath='{.status.initContainerStatuses[0].state.terminated.reason}' 2>/dev/null || echo "not-found")
if [ "$INIT_STATUS" = "Completed" ]; then
    echo -e "${GREEN}  ✓ InitContainer already completed${NC}"
else
    echo -e "${YELLOW}  ⚠ InitContainer status: ${INIT_STATUS}${NC}"
fi

echo ""
echo -e "${GREEN}[2/3] Testing pod lifecycle (delete and recreate)...${NC}"
echo "  Deleting pod: $INITIAL_POD"
kubectl delete pod -n "$NAMESPACE" "$INITIAL_POD" &> /dev/null

# Wait for new pod to be created
echo "  Waiting for new pod to be created..."
for i in {1..30}; do
    NEW_POD=$(kubectl get pods -n "$NAMESPACE" -l "$APP_LABEL" -o jsonpath='{.items[0].metadata.name}' 2>/dev/null)
    if [ -n "$NEW_POD" ] && [ "$NEW_POD" != "$INITIAL_POD" ]; then
        echo "  New pod created: $NEW_POD"
        break
    fi
    sleep 1
done

if [ -z "$NEW_POD" ] || [ "$NEW_POD" = "$INITIAL_POD" ]; then
    echo -e "${RED}Error: New pod not created within 30 seconds${NC}"
    exit 1
fi

# Wait for initContainer to complete
echo "  Waiting for initContainer to complete..."
for i in {1..30}; do
    INIT_STATUS=$(kubectl get pod -n "$NAMESPACE" "$NEW_POD" -o jsonpath='{.status.initContainerStatuses[0].state.terminated.reason}' 2>/dev/null || echo "running")
    POD_STATUS=$(kubectl get pod -n "$NAMESPACE" "$NEW_POD" -o jsonpath='{.status.phase}' 2>/dev/null || echo "Unknown")
    
    if [ "$INIT_STATUS" = "Completed" ]; then
        echo -e "${GREEN}  ✓ InitContainer completed${NC}"
        break
    fi
    
    if [ "$POD_STATUS" = "Failed" ] || [ "$POD_STATUS" = "Error" ]; then
        echo -e "${RED}Error: Pod failed with status: $POD_STATUS${NC}"
        kubectl describe pod -n "$NAMESPACE" "$NEW_POD" | tail -20
        exit 1
    fi
    
    sleep 1
done

if [ "$INIT_STATUS" != "Completed" ]; then
    echo -e "${RED}Error: InitContainer did not complete within 30 seconds${NC}"
    kubectl describe pod -n "$NAMESPACE" "$NEW_POD" | grep -A 10 "Init Containers"
    exit 1
fi

# Check initContainer exit code
EXIT_CODE=$(kubectl get pod -n "$NAMESPACE" "$NEW_POD" -o jsonpath='{.status.initContainerStatuses[0].state.terminated.exitCode}' 2>/dev/null || echo "unknown")
if [ "$EXIT_CODE" != "0" ]; then
    echo -e "${RED}Error: InitContainer exited with code $EXIT_CODE${NC}"
    kubectl logs -n "$NAMESPACE" "$NEW_POD" -c spiffe-helper
    exit 1
fi
echo -e "${GREEN}  ✓ InitContainer exited successfully (code: $EXIT_CODE)${NC}"

echo ""
echo -e "${GREEN}[3/3] Verifying main container started...${NC}"
# Wait for main container to be ready
for i in {1..30}; do
    MAIN_READY=$(kubectl get pod -n "$NAMESPACE" "$NEW_POD" -o jsonpath='{.status.containerStatuses[0].ready}' 2>/dev/null || echo "false")
    POD_STATUS=$(kubectl get pod -n "$NAMESPACE" "$NEW_POD" -o jsonpath='{.status.phase}' 2>/dev/null || echo "Unknown")
    
    if [ "$MAIN_READY" = "true" ] && [ "$POD_STATUS" = "Running" ]; then
        echo -e "${GREEN}  ✓ Main container is running and ready${NC}"
        break
    fi
    
    if [ "$POD_STATUS" = "Failed" ] || [ "$POD_STATUS" = "Error" ]; then
        echo -e "${RED}Error: Pod failed with status: $POD_STATUS${NC}"
        kubectl describe pod -n "$NAMESPACE" "$NEW_POD" | tail -20
        exit 1
    fi
    
    sleep 1
done

if [ "$MAIN_READY" != "true" ]; then
    echo -e "${RED}Error: Main container not ready within 30 seconds${NC}"
    kubectl describe pod -n "$NAMESPACE" "$NEW_POD" | grep -A 10 "Containers"
    exit 1
fi

# Check initContainer logs
echo ""
echo -e "${GREEN}=== InitContainer Logs ===${NC}"
kubectl logs -n "$NAMESPACE" "$NEW_POD" -c spiffe-helper 2>&1 | head -10

echo ""
echo -e "${GREEN}=== All Tests Passed! ===${NC}"
echo ""
echo "Summary:"
echo "  - InitContainer completed successfully"
echo "  - Main container started after initContainer"
echo "  - Pod lifecycle (delete/recreate) works correctly"
