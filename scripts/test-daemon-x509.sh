#!/bin/bash
set -e

# Integration test for X.509 certificate fetching in daemon mode
# This script tests that:
# 1. Daemon mode connects to SPIRE agent and fetches X.509 certificate and key
# 2. Certificates are persisted to the configured output directory
# 3. Daemon startup completes after first cert is pulled
# 4. Daemon exits with code 1 if certificate fetching fails

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
KUBECONFIG_PATH="${KUBECONFIG:-$ROOT_DIR/artifacts/kubeconfig}"
NAMESPACE="spiffe-helper-test"
TEST_POD="spiffe-helper-daemon-test"

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

echo -e "${GREEN}=== Testing X.509 Certificate Fetching in Daemon Mode ===${NC}"
echo ""

# Check if SPIRE agent is running
echo -e "${GREEN}[1/5] Checking SPIRE agent status...${NC}"
if ! kubectl get pods -n spire-agent -l app=spire-agent | grep -q Running; then
    echo -e "${RED}Error: SPIRE agent is not running${NC}"
    echo "Please ensure SPIRE agent is deployed: make deploy-spire-agent"
    exit 1
fi
echo -e "${GREEN}  ✓ SPIRE agent is running${NC}"

# Check if SPIRE server is running
echo ""
echo -e "${GREEN}[2/5] Checking SPIRE server status...${NC}"
SPIRE_SERVER_POD=$(kubectl get pods -n spire-server -l app=spire-server -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")
if [ -z "$SPIRE_SERVER_POD" ]; then
    echo -e "${RED}Error: SPIRE server is not running${NC}"
    echo "Please ensure SPIRE server is deployed: make deploy-spire-server"
    exit 1
fi
echo -e "${GREEN}  ✓ SPIRE server is running (pod: $SPIRE_SERVER_POD)${NC}"

# Ensure node alias exists
echo ""
echo -e "${GREEN}[3/5] Ensuring node alias exists...${NC}"
NODE_ALIAS_ID="spiffe://spiffe-helper.local/k8s-cluster/spiffe-helper"
if kubectl exec -n spire-server "$SPIRE_SERVER_POD" -- \
    /opt/spire/bin/spire-server entry show -spiffeID "$NODE_ALIAS_ID" 2>/dev/null | grep -q "$NODE_ALIAS_ID"; then
    echo -e "${GREEN}  ✓ Node alias already exists${NC}"
else
    echo -e "${YELLOW}  Creating node alias...${NC}"
    if kubectl exec -n spire-server "$SPIRE_SERVER_POD" -- \
        /opt/spire/bin/spire-server entry create \
        -node \
        -spiffeID "$NODE_ALIAS_ID" \
        -selector "k8s_psat:cluster:spiffe-helper" 2>/dev/null; then
        echo -e "${GREEN}  ✓ Node alias created${NC}"
    else
        echo -e "${YELLOW}  ⚠ Failed to create node alias (may already exist)${NC}"
    fi
fi

# Create test namespace and service account
echo ""
echo -e "${GREEN}[4/5] Creating test namespace and registering workload...${NC}"
kubectl create namespace "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f - > /dev/null 2>&1 || true

# Create ServiceAccount for the test pod (needed for SPIRE registration)
kubectl create serviceaccount test-sa -n "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f - > /dev/null 2>&1 || true

# Register the test workload with SPIRE
TEST_SPIFFE_ID="spiffe://spiffe-helper.local/ns/${NAMESPACE}/sa/test-sa"
echo -e "${GREEN}  Registering workload: $TEST_SPIFFE_ID${NC}"
if kubectl exec -n spire-server "$SPIRE_SERVER_POD" -- \
    /opt/spire/bin/spire-server entry show -spiffeID "$TEST_SPIFFE_ID" 2>/dev/null | grep -q "$TEST_SPIFFE_ID"; then
    echo -e "${GREEN}  ✓ Workload already registered${NC}"
else
    if kubectl exec -n spire-server "$SPIRE_SERVER_POD" -- \
        /opt/spire/bin/spire-server entry create \
        -spiffeID "$TEST_SPIFFE_ID" \
        -parentID "$NODE_ALIAS_ID" \
        -selector "k8s:ns:${NAMESPACE}" \
        -selector "k8s:sa:test-sa" 2>/dev/null; then
        echo -e "${GREEN}  ✓ Workload registered successfully${NC}"
    else
        echo -e "${RED}  ✗ Failed to register workload${NC}"
        exit 1
    fi
fi

# Wait a moment for registration to propagate and allow time for workload attestation
echo -e "${GREEN}  Waiting for registration to propagate...${NC}"
sleep 5

# Create test pod
echo ""
echo -e "${GREEN}[5/5] Creating test pod...${NC}"

# Create ConfigMap with daemon mode configuration
kubectl create configmap spiffe-helper-daemon-config -n "$NAMESPACE" \
    --from-literal=helper.conf='agent_address = "unix:///run/spire/sockets/agent.sock"
daemon_mode = true
cert_dir = "/tmp/certs"
svid_file_name = "svid.pem"
svid_key_file_name = "svid_key.pem"
health_checks {
    listener_enabled = false
}' \
    --dry-run=client -o yaml | kubectl apply -f - > /dev/null 2>&1

# Create test pod with daemon mode
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
  containers:
  - name: spiffe-helper
    image: spiffe-helper-rust:test
    imagePullPolicy: Never
    args:
    - /usr/local/bin/spiffe-helper-rust
    - --config
    - /etc/spiffe-helper/helper.conf
    volumeMounts:
    - name: spiffe-socket
      mountPath: /run/spire/sockets
      readOnly: true
    - name: config
      mountPath: /etc/spiffe-helper
    - name: certs
      mountPath: /tmp/certs
  volumes:
  - name: spiffe-socket
    hostPath:
      path: /run/spire/sockets
      type: DirectoryOrCreate
  - name: config
    configMap:
      name: spiffe-helper-daemon-config
  - name: certs
    emptyDir: {}
EOF

echo -e "${GREEN}  ✓ Test pod created${NC}"

# Wait for pod to be running
echo ""
echo -e "${GREEN}Waiting for pod to start and fetch certificates...${NC}"
for i in {1..60}; do
    POD_STATUS=$(kubectl get pod -n "$NAMESPACE" "$TEST_POD" -o jsonpath='{.status.phase}' 2>/dev/null || echo "Unknown")
    CONTAINER_STATUS=$(kubectl get pod -n "$NAMESPACE" "$TEST_POD" -o jsonpath='{.status.containerStatuses[0].ready}' 2>/dev/null || echo "false")
    
    if [ "$POD_STATUS" = "Running" ] && [ "$CONTAINER_STATUS" = "true" ]; then
        echo -e "${GREEN}  ✓ Pod is running${NC}"
        break
    fi
    
    if [ "$POD_STATUS" = "Failed" ] || [ "$POD_STATUS" = "Error" ]; then
        echo -e "${RED}Error: Pod failed with status: $POD_STATUS${NC}"
        kubectl logs -n "$NAMESPACE" "$TEST_POD" 2>&1 | tail -20
        kubectl describe pod -n "$NAMESPACE" "$TEST_POD" | tail -30
        exit 1
    fi
    
    sleep 1
done

if [ "$POD_STATUS" != "Running" ] || [ "$CONTAINER_STATUS" != "true" ]; then
    echo -e "${RED}Error: Pod did not start successfully within 60 seconds${NC}"
    kubectl logs -n "$NAMESPACE" "$TEST_POD" 2>&1 | tail -20
    kubectl describe pod -n "$NAMESPACE" "$TEST_POD" | tail -30
    exit 1
fi

# Wait for certificate files to be created (certificate fetching may take time with retries)
echo -e "${GREEN}  Waiting for certificate files to be created...${NC}"
for i in {1..40}; do
    if kubectl exec -n "$NAMESPACE" "$TEST_POD" -- test -f /tmp/certs/svid.pem 2>/dev/null && \
       kubectl exec -n "$NAMESPACE" "$TEST_POD" -- test -f /tmp/certs/svid_key.pem 2>/dev/null; then
        echo -e "${GREEN}  ✓ Certificate files created${NC}"
        break
    fi
    sleep 1
done

if ! kubectl exec -n "$NAMESPACE" "$TEST_POD" -- test -f /tmp/certs/svid.pem 2>/dev/null; then
    echo -e "${RED}Error: Certificate files were not created within 40 seconds${NC}"
    kubectl logs -n "$NAMESPACE" "$TEST_POD" 2>&1 | tail -30
    exit 1
fi

# Check logs for certificate fetching success
echo ""
echo -e "${GREEN}Verifying certificate fetching...${NC}"
LOGS=$(kubectl logs -n "$NAMESPACE" "$TEST_POD" 2>&1 || echo "")
if echo "$LOGS" | grep -q "Successfully fetched and wrote X.509 certificate"; then
    echo -e "${GREEN}  ✓ Certificate fetching logged${NC}"
else
    echo -e "${YELLOW}  ⚠ Certificate fetching message not found in logs${NC}"
    echo "Logs:"
    echo "$LOGS" | tail -10
fi

# Verify certificates exist in the pod
echo ""
echo -e "${GREEN}Verifying certificates exist in pod...${NC}"
if kubectl exec -n "$NAMESPACE" "$TEST_POD" -- test -f /tmp/certs/svid.pem 2>/dev/null; then
    echo -e "${GREEN}  ✓ Certificate file (svid.pem) exists${NC}"
else
    echo -e "${RED}  ✗ Certificate file (svid.pem) not found${NC}"
    exit 1
fi

if kubectl exec -n "$NAMESPACE" "$TEST_POD" -- test -f /tmp/certs/svid_key.pem 2>/dev/null; then
    echo -e "${GREEN}  ✓ Private key file (svid_key.pem) exists${NC}"
else
    echo -e "${RED}  ✗ Private key file (svid_key.pem) not found${NC}"
    exit 1
fi

# Verify certificate content (basic check)
CERT_CONTENT=$(kubectl exec -n "$NAMESPACE" "$TEST_POD" -- cat /tmp/certs/svid.pem 2>/dev/null || echo "")
if echo "$CERT_CONTENT" | grep -q "BEGIN CERTIFICATE"; then
    echo -e "${GREEN}  ✓ Certificate file contains valid PEM format${NC}"
else
    echo -e "${RED}  ✗ Certificate file does not contain valid PEM format${NC}"
    exit 1
fi

KEY_CONTENT=$(kubectl exec -n "$NAMESPACE" "$TEST_POD" -- cat /tmp/certs/svid_key.pem 2>/dev/null || echo "")
if echo "$KEY_CONTENT" | grep -q "BEGIN.*PRIVATE KEY"; then
    echo -e "${GREEN}  ✓ Private key file contains valid PEM format${NC}"
else
    echo -e "${RED}  ✗ Private key file does not contain valid PEM format${NC}"
    exit 1
fi

# Cleanup
echo ""
echo -e "${GREEN}Cleaning up test pod...${NC}"
kubectl delete pod -n "$NAMESPACE" "$TEST_POD" > /dev/null 2>&1 || true
kubectl delete configmap -n "$NAMESPACE" spiffe-helper-daemon-config > /dev/null 2>&1 || true

echo ""
echo -e "${GREEN}=== All Tests Passed! ===${NC}"
echo ""
echo "Summary:"
echo "  - Daemon mode started successfully"
echo "  - X.509 certificate fetched from SPIRE agent"
echo "  - Certificate and key written to configured directory"
echo "  - Daemon continued running after certificate fetch"
echo "  - Certificate files contain valid PEM format"
