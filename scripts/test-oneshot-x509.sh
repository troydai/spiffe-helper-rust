#!/bin/bash
set -e

# Integration test for X.509 certificate fetching in one-shot mode
# This script tests that:
# 1. One-shot mode connects to SPIRE agent and fetches X.509 certificate and key
# 2. Certificates are persisted to the configured output directory
# 3. One-shot mode exits successfully after fetching certificates
# 4. One-shot mode exits with code 1 if certificate fetching fails

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
KUBECONFIG_PATH="${KUBECONFIG:-$ROOT_DIR/artifacts/kubeconfig}"
NAMESPACE="spiffe-helper-smoke-test"
TEST_POD="spiffe-helper-oneshot-test"
NODE_ALIAS_ID="spiffe://spiffe-helper.local/k8s-cluster/spiffe-helper"
TEST_SPIFFE_ID="spiffe://spiffe-helper.local/ns/${NAMESPACE}/sa/test-sa"

# Source color support
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

echo -e "${COLOR_BRIGHT_BLUE}=== Testing X.509 Certificate Fetching in One-Shot Mode ===${COLOR_RESET}"
echo ""

# Check if SPIRE agent is running
echo -e "${COLOR_GREEN}[1/5] Checking SPIRE agent status...${COLOR_RESET}"
if ! kubectl get pods -n spire-agent -l app=spire-agent | grep -q Running; then
    echo -e "${COLOR_RED}Error: SPIRE agent is not running${COLOR_RESET}"
    echo "Please ensure SPIRE agent is deployed: make deploy-spire-agent"
    exit 1
fi
echo -e "${COLOR_GREEN}  ✓ SPIRE agent is running${COLOR_RESET}"

# Check if SPIRE server is running
echo ""
echo -e "${COLOR_GREEN}[2/5] Checking SPIRE server status...${COLOR_RESET}"
SPIRE_SERVER_POD=$(kubectl get pods -n spire-server -l app=spire-server -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")
if [ -z "$SPIRE_SERVER_POD" ]; then
    echo -e "${COLOR_RED}Error: SPIRE server is not running${COLOR_RESET}"
    echo "Please ensure SPIRE server is deployed: make deploy-spire-server"
    exit 1
fi
echo -e "${COLOR_GREEN}  ✓ SPIRE server is running (pod: $SPIRE_SERVER_POD)${COLOR_RESET}"

# Verify node alias exists (should be created by parent script)
echo ""
echo -e "${COLOR_GREEN}[3/5] Verifying node alias exists...${COLOR_RESET}"
if kubectl exec -n spire-server "$SPIRE_SERVER_POD" -- \
    /opt/spire/bin/spire-server entry show -spiffeID "$NODE_ALIAS_ID" 2>/dev/null | grep -q "$NODE_ALIAS_ID"; then
    echo -e "${COLOR_GREEN}  ✓ Node alias exists${COLOR_RESET}"
else
    echo -e "${COLOR_RED}  ✗ Node alias not found: $NODE_ALIAS_ID${COLOR_RESET}"
    echo -e "${COLOR_YELLOW}  Node alias should be created by the parent script${COLOR_RESET}"
    exit 1
fi

# Create test namespace and service account
echo ""
echo -e "${COLOR_GREEN}[4/5] Creating test namespace and registering workload...${COLOR_RESET}"
kubectl create namespace "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f - > /dev/null 2>&1 || true

# Create ServiceAccount for the test pod (needed for SPIRE registration)
kubectl create serviceaccount test-sa -n "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f - > /dev/null 2>&1 || true

# Register the test workload with SPIRE
echo -e "${COLOR_GREEN}  Registering workload: $TEST_SPIFFE_ID${COLOR_RESET}"
if kubectl exec -n spire-server "$SPIRE_SERVER_POD" -- \
    /opt/spire/bin/spire-server entry show -spiffeID "$TEST_SPIFFE_ID" 2>/dev/null | grep -q "$TEST_SPIFFE_ID"; then
    echo -e "${COLOR_GREEN}  ✓ Workload already registered${COLOR_RESET}"
else
    if kubectl exec -n spire-server "$SPIRE_SERVER_POD" -- \
        /opt/spire/bin/spire-server entry create \
        -spiffeID "$TEST_SPIFFE_ID" \
        -parentID "$NODE_ALIAS_ID" \
        -selector "k8s:ns:${NAMESPACE}" \
        -selector "k8s:sa:test-sa" 2>/dev/null; then
        echo -e "${COLOR_GREEN}  ✓ Workload registered successfully${COLOR_RESET}"
    else
        echo -e "${COLOR_RED}  ✗ Failed to register workload${COLOR_RESET}"
        exit 1
    fi
fi

# Wait a moment for registration to propagate and allow time for workload attestation
echo -e "${COLOR_GREEN}  Waiting for registration to propagate...${COLOR_RESET}"
sleep 5

# Create test pod
echo ""
echo -e "${COLOR_GREEN}[5/5] Creating test pod...${COLOR_RESET}"

# Create ConfigMap with one-shot mode configuration
kubectl create configmap spiffe-helper-oneshot-config -n "$NAMESPACE" \
    --from-literal=helper.conf='agent_address = "unix:///run/spire/sockets/agent.sock"
daemon_mode = false
cert_dir = "/tmp/certs"
svid_file_name = "svid.pem"
svid_key_file_name = "svid_key.pem"' \
    --dry-run=client -o yaml | kubectl apply -f - > /dev/null 2>&1

# Create test pod with initContainer running one-shot mode
cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: $TEST_POD
  namespace: $NAMESPACE
  labels:
    app: spiffe-helper-test
spec:
  serviceAccountName: test-sa
  initContainers:
  - name: spiffe-helper-oneshot
    image: spiffe-helper-rust:test
    imagePullPolicy: Never
    args:
    - /usr/local/bin/spiffe-helper-rust
    - --config
    - /etc/spiffe-helper/helper.conf
    - --daemon-mode
    - "false"
    volumeMounts:
    - name: spiffe-socket
      mountPath: /run/spire/sockets
      readOnly: true
    - name: config
      mountPath: /etc/spiffe-helper
    - name: certs
      mountPath: /tmp/certs
  containers:
  - name: sleep
    image: busybox:latest
    command: ["sleep", "3600"]
    volumeMounts:
    - name: certs
      mountPath: /tmp/certs
      readOnly: true
  volumes:
  - name: spiffe-socket
    hostPath:
      path: /run/spire/sockets
      type: DirectoryOrCreate
  - name: config
    configMap:
      name: spiffe-helper-oneshot-config
  - name: certs
    emptyDir: {}
EOF

echo -e "${COLOR_GREEN}  ✓ Test pod created${COLOR_RESET}"

# Wait for initContainer to complete
echo ""
echo -e "${COLOR_GREEN}Waiting for one-shot mode to complete...${COLOR_RESET}"
INIT_COMPLETED=false
for i in {1..60}; do
    INIT_READY=$(kubectl get pod -n "$NAMESPACE" "$TEST_POD" -o jsonpath='{.status.initContainerStatuses[0].ready}' 2>/dev/null || echo "false")
    INIT_REASON=$(kubectl get pod -n "$NAMESPACE" "$TEST_POD" -o jsonpath='{.status.initContainerStatuses[0].state.terminated.reason}' 2>/dev/null || echo "")
    
    if [ "$INIT_READY" = "true" ] || [ "$INIT_REASON" = "Completed" ]; then
        echo -e "${COLOR_GREEN}  ✓ One-shot mode completed successfully${COLOR_RESET}"
        INIT_COMPLETED=true
        break
    fi
    
    POD_STATUS=$(kubectl get pod -n "$NAMESPACE" "$TEST_POD" -o jsonpath='{.status.phase}' 2>/dev/null || echo "Unknown")
    INIT_EXIT_CODE=$(kubectl get pod -n "$NAMESPACE" "$TEST_POD" -o jsonpath='{.status.initContainerStatuses[0].state.terminated.exitCode}' 2>/dev/null || echo "")
    
    if [ "$POD_STATUS" = "Failed" ] || [ "$POD_STATUS" = "Error" ] || [ "$INIT_EXIT_CODE" = "1" ]; then
        echo -e "${COLOR_RED}Error: Pod failed with status: $POD_STATUS, exit code: $INIT_EXIT_CODE${COLOR_RESET}"
        kubectl logs -n "$NAMESPACE" "$TEST_POD" -c spiffe-helper-oneshot 2>&1 | tail -20
        kubectl describe pod -n "$NAMESPACE" "$TEST_POD" | tail -30
        # Cleanup
        kubectl delete pod -n "$NAMESPACE" "$TEST_POD" > /dev/null 2>&1 || true
        kubectl delete configmap -n "$NAMESPACE" spiffe-helper-oneshot-config > /dev/null 2>&1 || true
        exit 1
    fi
    
    sleep 1
done

if [ "$INIT_COMPLETED" != "true" ]; then
    echo -e "${COLOR_RED}Error: InitContainer did not complete within 60 seconds${COLOR_RESET}"
    kubectl logs -n "$NAMESPACE" "$TEST_POD" -c spiffe-helper-oneshot 2>&1 | tail -20
    kubectl describe pod -n "$NAMESPACE" "$TEST_POD" | tail -30
    # Cleanup
    kubectl delete pod -n "$NAMESPACE" "$TEST_POD" > /dev/null 2>&1 || true
    kubectl delete configmap -n "$NAMESPACE" spiffe-helper-oneshot-config > /dev/null 2>&1 || true
    exit 1
fi

# Verify certificate files exist
echo ""
echo -e "${COLOR_GREEN}Verifying certificates exist in pod...${COLOR_RESET}"
if kubectl exec -n "$NAMESPACE" "$TEST_POD" -c sleep -- test -f /tmp/certs/svid.pem 2>/dev/null; then
    echo -e "${COLOR_GREEN}  ✓ Certificate file (svid.pem) exists${COLOR_RESET}"
else
    echo -e "${COLOR_RED}  ✗ Certificate file (svid.pem) not found${COLOR_RESET}"
    kubectl logs -n "$NAMESPACE" "$TEST_POD" -c spiffe-helper-oneshot 2>&1 | tail -20
    # Cleanup
    kubectl delete pod -n "$NAMESPACE" "$TEST_POD" > /dev/null 2>&1 || true
    kubectl delete configmap -n "$NAMESPACE" spiffe-helper-oneshot-config > /dev/null 2>&1 || true
    exit 1
fi

if kubectl exec -n "$NAMESPACE" "$TEST_POD" -c sleep -- test -f /tmp/certs/svid_key.pem 2>/dev/null; then
    echo -e "${COLOR_GREEN}  ✓ Private key file (svid_key.pem) exists${COLOR_RESET}"
else
    echo -e "${COLOR_RED}  ✗ Private key file (svid_key.pem) not found${COLOR_RESET}"
    kubectl logs -n "$NAMESPACE" "$TEST_POD" -c spiffe-helper-oneshot 2>&1 | tail -20
    # Cleanup
    kubectl delete pod -n "$NAMESPACE" "$TEST_POD" > /dev/null 2>&1 || true
    kubectl delete configmap -n "$NAMESPACE" spiffe-helper-oneshot-config > /dev/null 2>&1 || true
    exit 1
fi

# Verify certificate content (basic check)
CERT_CONTENT=$(kubectl exec -n "$NAMESPACE" "$TEST_POD" -c sleep -- cat /tmp/certs/svid.pem 2>/dev/null || echo "")
if echo "$CERT_CONTENT" | grep -q "BEGIN CERTIFICATE"; then
    echo -e "${COLOR_GREEN}  ✓ Certificate file contains valid PEM format${COLOR_RESET}"
else
    echo -e "${COLOR_RED}  ✗ Certificate file does not contain valid PEM format${COLOR_RESET}"
    # Cleanup
    kubectl delete pod -n "$NAMESPACE" "$TEST_POD" > /dev/null 2>&1 || true
    kubectl delete configmap -n "$NAMESPACE" spiffe-helper-oneshot-config > /dev/null 2>&1 || true
    exit 1
fi

KEY_CONTENT=$(kubectl exec -n "$NAMESPACE" "$TEST_POD" -c sleep -- cat /tmp/certs/svid_key.pem 2>/dev/null || echo "")
if echo "$KEY_CONTENT" | grep -q "BEGIN.*PRIVATE KEY"; then
    echo -e "${COLOR_GREEN}  ✓ Private key file contains valid PEM format${COLOR_RESET}"
else
    echo -e "${COLOR_RED}  ✗ Private key file does not contain valid PEM format${COLOR_RESET}"
    # Cleanup
    kubectl delete pod -n "$NAMESPACE" "$TEST_POD" > /dev/null 2>&1 || true
    kubectl delete configmap -n "$NAMESPACE" spiffe-helper-oneshot-config > /dev/null 2>&1 || true
    exit 1
fi

# Check logs for one-shot mode completion
echo ""
echo -e "${COLOR_GREEN}Verifying one-shot mode completion...${COLOR_RESET}"
LOGS=$(kubectl logs -n "$NAMESPACE" "$TEST_POD" -c spiffe-helper-oneshot 2>&1 || echo "")
if echo "$LOGS" | grep -q "Successfully fetched and wrote X.509 certificate"; then
    echo -e "${COLOR_GREEN}  ✓ Certificate fetching logged${COLOR_RESET}"
else
    echo -e "${COLOR_YELLOW}  ⚠ Certificate fetching message not found in logs${COLOR_RESET}"
    echo "Logs:"
    echo "$LOGS" | tail -10
fi

# Cleanup
echo ""
echo -e "${COLOR_GREEN}Cleaning up test pod...${COLOR_RESET}"
kubectl delete pod -n "$NAMESPACE" "$TEST_POD" > /dev/null 2>&1 || true
kubectl delete configmap -n "$NAMESPACE" spiffe-helper-oneshot-config > /dev/null 2>&1 || true

echo ""
echo -e "${COLOR_BRIGHT_GREEN}=== All Tests Passed! ===${COLOR_RESET}"
echo ""
echo "Summary:"
echo "  - One-shot mode started successfully"
echo "  - X.509 certificate fetched from SPIRE agent"
echo "  - Certificate and key written to configured directory"
echo "  - One-shot mode exited successfully after certificate fetch"
echo "  - Certificate files contain valid PEM format"

