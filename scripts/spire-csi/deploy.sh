#!/bin/bash
# Deploys the SPIRE CSI driver to the Kubernetes cluster.

set -e

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
ROOT_DIR="${ROOT_DIR:-$(cd "${DIR}/../.." && pwd)}"
KUBECONFIG_PATH="${KUBECONFIG_PATH:-${ROOT_DIR}/artifacts/kubeconfig}"

export KUBECONFIG="${KUBECONFIG_PATH}"

echo "Deploying SPIRE CSI driver..."
kubectl apply -f "${DIR}/../deploy/spire/csi/"
echo "SPIRE CSI driver deployed."
