#!/bin/bash
# Integration test for PID file signaling feature.
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
KUBECONFIG_PATH="${KUBECONFIG:-$ROOT_DIR/artifacts/kubeconfig}"
NAMESPACE="spiffe-helper-test-pid"
TEST_POD="spiffe-helper-pid"

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
    kubectl delete configmap -n "$NAMESPACE" spiffe-helper-pid-config --ignore-not-found > /dev/null 2>&1 || true
    kubectl delete serviceaccount -n "$NAMESPACE" test-sa --ignore-not-found > /dev/null 2>&1 || true
    kubectl delete namespace "$NAMESPACE" --ignore-not-found > /dev/null 2>&1 || true
    rm -f helper.conf
}

trap cleanup EXIT

if [ ! -f "$KUBECONFIG_PATH" ]; then
    echo -e "${RED}Error: Kubeconfig not found at $KUBECONFIG_PATH${NC}"
    exit 1
fi

export KUBECONFIG="$KUBECONFIG_PATH"

echo -e "${GREEN}=== Testing PID File Signaling ===${NC}"

# Ensure namespace and SA exist
kubectl create namespace "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f - > /dev/null 2>&1 || true
kubectl create serviceaccount test-sa -n "$NAMESPACE" --dry-run=client -o yaml | kubectl apply -f - > /dev/null 2>&1 || true

# Register workload
SPIRE_SERVER_POD=$(kubectl get pods -n spire-server -l app=spire-server -o jsonpath='{.items[0].metadata.name}')
NODE_ALIAS_ID="spiffe://spiffe-helper.local/k8s-cluster/spiffe-helper"
TEST_SPIFFE_ID="spiffe://spiffe-helper.local/ns/${NAMESPACE}/sa/test-sa"

kubectl exec -n spire-server "$SPIRE_SERVER_POD" -- \
    /opt/spire/bin/spire-server entry create \
    -spiffeID "$TEST_SPIFFE_ID" \
    -parentID "$NODE_ALIAS_ID" \
    -selector "k8s:ns:${NAMESPACE}" \
    -selector "k8s:sa:test-sa" > /dev/null 2>&1 || true

# Create configuration file
cat <<EOF > helper.conf
agent_address = "unix:///run/spire/sockets/agent.sock"
daemon_mode = true
cert_dir = "/tmp/certs"
cmd = "/bin/sh"
cmd_args = "-c \"echo \$\$ > /tmp/my-app.pid; trap 'echo RECEIVED_SIGUSR2_VIA_PID_FILE' USR2; echo PID_PROCESS_STARTED; while true; do sleep 1; done\""
pid_file_name = "/tmp/my-app.pid"
renew_signal = "SIGUSR2"
health_checks {
    listener_enabled = false
}
EOF

# Create ConfigMap from file
kubectl create configmap spiffe-helper-pid-config -n "$NAMESPACE" \
    --from-file=helper.conf \
    --dry-run=client -o yaml | kubectl apply -f - > /dev/null 2>&1

# Create Pod
cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: $TEST_POD
  namespace: $NAMESPACE
spec:
  serviceAccountName: test-sa
  containers:
  - name: spiffe-helper
    image: spiffe-helper-rust:test
    imagePullPolicy: Never
    args: ["/usr/local/bin/spiffe-helper-rust", "--config", "/etc/spiffe-helper/helper.conf"]
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
      name: spiffe-helper-pid-config
  - name: certs
    emptyDir: {}
EOF

echo -e "${CYAN}Waiting for pod to start and PID file to be written...${NC}"
for i in {1..60}; do
    if kubectl logs -n "$NAMESPACE" "$TEST_POD" 2>/dev/null | grep -q "PID_PROCESS_STARTED"; then
        echo -e "${GREEN}  ✓ PID-based process started${NC}"
        break
    fi
    sleep 1
    if [ $i -eq 60 ]; then
        echo -e "${RED}Error: Process did not start in time${NC}"
        kubectl logs -n "$NAMESPACE" "$TEST_POD"
        exit 1
    fi
done

# Wait for rotation
echo -e "${CYAN}Waiting for certificate rotation to trigger signal...${NC}"
for i in {1..120}; do
    LOGS=$(kubectl logs -n "$NAMESPACE" "$TEST_POD" 2>/dev/null)
    if echo "$LOGS" | grep -q "RECEIVED_SIGUSR2_VIA_PID_FILE"; then
        echo -e "${GREEN}  ✓ Process received SIGUSR2 via PID file on certificate rotation${NC}"
        exit 0
    fi
    sleep 2
done

echo -e "${RED}Error: Process did not receive signal via PID file within timeout${NC}"
kubectl logs -n "$NAMESPACE" "$TEST_POD"
exit 1