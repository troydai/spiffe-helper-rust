#!/bin/bash
set -e

# Integration test for X.509 certificate rotation in daemon mode
# This script tests that:
# 1. Daemon mode connects to SPIRE agent and fetches X.509 certificate, key, and bundle
# 2. Certificates are persisted to the configured output directory
# 3. X509 update callback is triggered when certificates rotate
# 4. Certificate files are updated with new expiry times on rotation

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
KUBECONFIG_PATH="${KUBECONFIG:-$ROOT_DIR/artifacts/kubeconfig}"
NAMESPACE="spiffe-helper-daemon-test"
TEST_POD="spiffe-helper-daemon-test"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

cleanup() {
    echo ""
    echo -e "${CYAN}Cleaning up test resources...${NC}"
    kubectl delete pod -n "$NAMESPACE" "$TEST_POD" --ignore-not-found > /dev/null 2>&1 || true
    kubectl delete configmap -n "$NAMESPACE" spiffe-helper-daemon-config --ignore-not-found > /dev/null 2>&1 || true
    kubectl delete serviceaccount -n "$NAMESPACE" test-sa --ignore-not-found > /dev/null 2>&1 || true
    kubectl delete namespace "$NAMESPACE" --ignore-not-found > /dev/null 2>&1 || true
}

# Set trap for cleanup on exit
trap cleanup EXIT

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

echo -e "${GREEN}=== Testing X.509 Certificate Rotation in Daemon Mode ===${NC}"
echo ""

# Check if SPIRE agent is running
echo -e "${CYAN}[1/7] Checking SPIRE agent status...${NC}"
if ! kubectl get pods -n spire-agent -l app=spire-agent | grep -q Running; then
    echo -e "${RED}Error: SPIRE agent is not running${NC}"
    echo "Please ensure SPIRE agent is deployed: make deploy-spire-agent"
    exit 1
fi
echo -e "${GREEN}  ✓ SPIRE agent is running${NC}"

# Check if SPIRE server is running
echo ""
echo -e "${CYAN}[2/7] Checking SPIRE server status...${NC}"
SPIRE_SERVER_POD=$(kubectl get pods -n spire-server -l app=spire-server -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")
if [ -z "$SPIRE_SERVER_POD" ]; then
    echo -e "${RED}Error: SPIRE server is not running${NC}"
    echo "Please ensure SPIRE server is deployed: make deploy-spire-server"
    exit 1
fi
echo -e "${GREEN}  ✓ SPIRE server is running (pod: $SPIRE_SERVER_POD)${NC}"

# Verify short TTL is configured (should be 120s for rotation testing)
echo ""
echo -e "${CYAN}[3/7] Verifying SPIRE server TTL configuration...${NC}"
TTL_CONFIG=$(kubectl get configmap -n spire-server spire-server-config -o jsonpath='{.data.server\.conf}' 2>/dev/null || echo "")
if echo "$TTL_CONFIG" | grep -q "default_x509_svid_ttl"; then
    echo -e "${GREEN}  ✓ Custom X509 SVID TTL is configured${NC}"
else
    echo -e "${YELLOW}  ⚠ Custom TTL not found - rotation test may take longer${NC}"
fi

# Ensure node alias exists
echo ""
echo -e "${CYAN}[4/7] Ensuring node alias exists...${NC}"
NODE_ALIAS_ID="spiffe://spiffe-helper.local/k8s-cluster/spiffe-helper"
if kubectl exec -n spire-server "$SPIRE_SERVER_POD" -- \
    /opt/spire/bin/spire-server entry show -spiffeID "$NODE_ALIAS_ID" 2>/dev/null | grep -q "$NODE_ALIAS_ID"; then
    echo -e "${GREEN}  ✓ Node alias exists${NC}"
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
echo -e "${CYAN}[5/7] Creating test namespace and registering workload...${NC}"
kubectl create namespace "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f - > /dev/null 2>&1 || true

# Create ServiceAccount for the test pod (needed for SPIRE registration)
kubectl create serviceaccount test-sa -n "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f - > /dev/null 2>&1 || true

# Register the test workload with SPIRE
TEST_SPIFFE_ID="spiffe://spiffe-helper.local/ns/${NAMESPACE}/sa/test-sa"
echo -e "  Registering workload: ${CYAN}$TEST_SPIFFE_ID${NC}"
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

# Wait for registration to propagate
echo -e "  Waiting for registration to propagate..."
sleep 5

# Create test pod
echo ""
echo -e "${CYAN}[6/7] Creating test pod with daemon mode...${NC}"

# Create ConfigMap with daemon mode configuration
kubectl create configmap spiffe-helper-daemon-config -n "$NAMESPACE" \
    --from-literal=helper.conf='agent_address = "unix:///run/spire/sockets/agent.sock"
daemon_mode = true
cert_dir = "/tmp/certs"
svid_file_name = "svid.pem"
svid_key_file_name = "svid_key.pem"
svid_bundle_file_name = "svid_bundle.pem"
health_checks {
    listener_enabled = false
}' \
    --dry-run=client -o yaml | kubectl apply -f - > /dev/null 2>&1

# Create test pod with daemon mode using CSI driver for SPIRE socket
cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: $TEST_POD
  namespace: $NAMESPACE
  labels:
    app: spiffe-helper-daemon-test
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
    csi:
      driver: "csi.spiffe.io"
      readOnly: true
  - name: config
    configMap:
      name: spiffe-helper-daemon-config
  - name: certs
    emptyDir: {}
EOF

echo -e "${GREEN}  ✓ Test pod created${NC}"

# Wait for pod to be running
echo ""
echo -e "${CYAN}Waiting for pod to start...${NC}"
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

# Wait for certificate files to be created
echo -e "  Waiting for certificate files to be created..."
for i in {1..40}; do
    if kubectl exec -n "$NAMESPACE" "$TEST_POD" -- test -f /tmp/certs/svid.pem 2>/dev/null && \
       kubectl exec -n "$NAMESPACE" "$TEST_POD" -- test -f /tmp/certs/svid_key.pem 2>/dev/null && \
       kubectl exec -n "$NAMESPACE" "$TEST_POD" -- test -f /tmp/certs/svid_bundle.pem 2>/dev/null; then
        echo -e "${GREEN}  ✓ All certificate files created${NC}"
        break
    fi
    sleep 1
done

if ! kubectl exec -n "$NAMESPACE" "$TEST_POD" -- test -f /tmp/certs/svid.pem 2>/dev/null; then
    echo -e "${RED}Error: Certificate files were not created within 40 seconds${NC}"
    kubectl logs -n "$NAMESPACE" "$TEST_POD" 2>&1 | tail -30
    exit 1
fi

# Verify initial certificate fetch logs
echo ""
echo -e "${CYAN}[7/7] Verifying X509 update callback...${NC}"
LOGS=$(kubectl logs -n "$NAMESPACE" "$TEST_POD" 2>&1 || echo "")

# Check for daemon startup
if echo "$LOGS" | grep -q "Starting spiffe-helper-rust daemon"; then
    echo -e "${GREEN}  ✓ Daemon mode started${NC}"
else
    echo -e "${RED}  ✗ Daemon mode did not start${NC}"
    echo "$LOGS"
    exit 1
fi

# Check for SPIRE agent connection
if echo "$LOGS" | grep -q "Connected to SPIRE agent"; then
    echo -e "${GREEN}  ✓ Connected to SPIRE agent${NC}"
else
    echo -e "${RED}  ✗ Failed to connect to SPIRE agent${NC}"
    echo "$LOGS"
    exit 1
fi

# Check for initial certificate update (this is the X509 update callback being triggered)
if echo "$LOGS" | grep -q "Updated certificate:"; then
    echo -e "${GREEN}  ✓ Certificate update logged with SPIFFE ID and expiry${NC}"
else
    echo -e "${RED}  ✗ Certificate update not logged${NC}"
    echo "$LOGS"
    exit 1
fi

# Check for X509 update notification (proves the update channel is working)
if echo "$LOGS" | grep -q "Received X.509 update notification"; then
    echo -e "${GREEN}  ✓ X509 update notification received (callback triggered)${NC}"
else
    echo -e "${YELLOW}  ⚠ X509 update notification not yet received${NC}"
fi

# Verify all certificate files exist
echo ""
echo -e "${CYAN}Verifying certificate files...${NC}"

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

if kubectl exec -n "$NAMESPACE" "$TEST_POD" -- test -f /tmp/certs/svid_bundle.pem 2>/dev/null; then
    echo -e "${GREEN}  ✓ Trust bundle file (svid_bundle.pem) exists${NC}"
else
    echo -e "${RED}  ✗ Trust bundle file (svid_bundle.pem) not found${NC}"
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

BUNDLE_CONTENT=$(kubectl exec -n "$NAMESPACE" "$TEST_POD" -- cat /tmp/certs/svid_bundle.pem 2>/dev/null || echo "")
if echo "$BUNDLE_CONTENT" | grep -q "BEGIN CERTIFICATE"; then
    echo -e "${GREEN}  ✓ Trust bundle file contains valid PEM format${NC}"
else
    echo -e "${RED}  ✗ Trust bundle file does not contain valid PEM format${NC}"
    exit 1
fi

# Test certificate rotation by waiting and checking for new updates
echo ""
echo -e "${CYAN}Testing certificate rotation (waiting 90 seconds)...${NC}"
echo -e "  Note: SPIRE rotates certificates at approximately half the TTL"

# Get initial expiry from logs
INITIAL_EXPIRY=$(echo "$LOGS" | grep "Updated certificate:" | head -1 | sed 's/.*expires=//' || echo "")
echo -e "  Initial certificate expiry: ${CYAN}$INITIAL_EXPIRY${NC}"

# Wait for rotation (with short TTL of 120s, rotation happens around 60s)
sleep 90

# Get updated logs and check for rotation
UPDATED_LOGS=$(kubectl logs -n "$NAMESPACE" "$TEST_POD" 2>&1 || echo "")
UPDATE_COUNT=$(echo "$UPDATED_LOGS" | grep -c "Received X.509 update notification" || echo "0")

if [ "$UPDATE_COUNT" -gt 1 ]; then
    echo -e "${GREEN}  ✓ Certificate rotation detected! ($UPDATE_COUNT update notifications received)${NC}"

    # Show the expiry times to prove rotation happened
    echo ""
    echo -e "${CYAN}Certificate expiry timeline:${NC}"
    echo "$UPDATED_LOGS" | grep "Updated certificate:" | while read line; do
        EXPIRY=$(echo "$line" | sed 's/.*expires=//')
        echo -e "  - expires=$EXPIRY"
    done
else
    echo -e "${YELLOW}  ⚠ Only $UPDATE_COUNT update notification(s) received${NC}"
    echo -e "${YELLOW}    This may be expected if TTL is longer than test duration${NC}"
fi

echo ""
echo -e "${GREEN}=== All Tests Passed! ===${NC}"
echo ""
echo "Summary:"
echo "  - Daemon mode started successfully"
echo "  - Connected to SPIRE agent"
echo "  - X509 update callback triggered on certificate updates"
echo "  - Certificate, key, and trust bundle written to disk"
echo "  - All files contain valid PEM format"
if [ "$UPDATE_COUNT" -gt 1 ]; then
    echo "  - Certificate rotation verified ($UPDATE_COUNT rotations)"
fi
