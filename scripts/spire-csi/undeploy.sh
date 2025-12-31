#!/bin/bash
# Removes the SPIRE CSI driver from the cluster.

set -e

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
ROOT_DIR="${ROOT_DIR:-$(cd "${DIR}/../.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo "Undeploying SPIRE CSI driver..."
kubectl delete -f "${DIR}/../deploy/spire/csi/" --ignore-not-found
echo "SPIRE CSI driver undeployed."
