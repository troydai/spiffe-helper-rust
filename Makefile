ROOT_DIR := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
KIND_CLUSTER_NAME ?= spiffe-helper
KIND_CONFIG := $(ROOT_DIR)/kind-config.yaml
ARTIFACTS_DIR := $(ROOT_DIR)/artifacts
KUBECONFIG_PATH := $(ARTIFACTS_DIR)/kubeconfig
CERT_DIR := $(ARTIFACTS_DIR)/certs
KUBECTL := KUBECONFIG="$(KUBECONFIG_PATH)" kubectl

.PHONY: tools
tools:
	@$(ROOT_DIR)/scripts/install-tools.sh

.PHONY: certs
certs:
	@$(ROOT_DIR)/scripts/generate-certs.sh

.PHONY: clean
clean:
	@echo "[clean] Removing generated artifacts..."
	@rm -rf $(ARTIFACTS_DIR)
	@if [ -d "$(ROOT_DIR)/bin" ]; then \
		echo "[clean] Removing binaries..." \
		rm -rf "$(ROOT_DIR)/bin"; \
	fi
	@echo "[clean] Clean complete."

.PHONY: cluster-up
cluster-up: $(KIND_CONFIG)
	@KIND="$(KIND)" KIND_CLUSTER_NAME="$(KIND_CLUSTER_NAME)" KIND_CONFIG="$(KIND_CONFIG)" ARTIFACTS_DIR="$(ARTIFACTS_DIR)" KUBECONFIG_PATH="$(KUBECONFIG_PATH)" ROOT_DIR="$(ROOT_DIR)" $(ROOT_DIR)/scripts/cluster-up.sh

.PHONY: cluster-down
cluster-down:
	@if kind get clusters | grep -qx "$(KIND_CLUSTER_NAME)"; then \
		echo "[cluster-down] Deleting kind cluster '$(KIND_CLUSTER_NAME)'" \
		kind delete cluster --name "$(KIND_CLUSTER_NAME)"; \
	else \
		echo "[cluster-down] kind cluster '$(KIND_CLUSTER_NAME)' already absent" \
	fi
	@rm -f "$(KUBECONFIG_PATH)"
	@if [ -d "$(ARTIFACTS_DIR)" ] && [ -z "$$($(KUBECTL) ls -A "$(ARTIFACTS_DIR)")" ]; then rmdir "$(ARTIFACTS_DIR)"; fi

# Check if cluster exists and kubeconfig is valid
.PHONY: check-cluster
check-cluster:
	@if ! kind get clusters | grep -qx "$(KIND_CLUSTER_NAME)"; then \
		echo "[check-cluster] Error: kind cluster '$(KIND_CLUSTER_NAME)' does not exist. Run 'make cluster-up' first." \
		exit 1; \
	fi
	@if [ ! -f "$(KUBECONFIG_PATH)" ]; then \
		echo "[check-cluster] Error: kubeconfig not found at $(KUBECONFIG_PATH). Run 'make cluster-up' first." \
		exit 1; \
	fi
	@if ! $(KUBECTL) cluster-info > /dev/null 2>&1; then \
		echo "[check-cluster] Error: unable to connect to cluster. Run 'make cluster-up' first." \
		exit 1; \
	fi

# Check if required certificate files exist
.PHONY: check-certs
check-certs:
	@if [ ! -f "$(CERT_DIR)/ca-cert.pem" ] || [ ! -f "$(CERT_DIR)/ca-key.pem" ]; then \
		echo "[check-certs] Error: CA certificate files not found. Expected: $(CERT_DIR)/ca-cert.pem, $(CERT_DIR)/ca-key.pem" \
		echo "[check-certs] Run 'make certs' first."; \
		exit 1; \
	fi
	@if [ ! -f "$(CERT_DIR)/spire-server-cert.pem" ] || [ ! -f "$(CERT_DIR)/spire-server-key.pem" ]; then \
		echo "[check-certs] Error: SPIRE server certificate files not found. Expected: $(CERT_DIR)/spire-server-cert.pem, $(CERT_DIR)/spire-server-key.pem" \
		echo "[check-certs] Run 'make certs' first."; \
		exit 1; \
	fi
	@if [ ! -f "$(CERT_DIR)/bootstrap-bundle.pem" ]; then \
		echo "[check-certs] Error: Bootstrap bundle not found. Expected: $(CERT_DIR)/bootstrap-bundle.pem" \
		echo "[check-certs] Run 'make certs' first."; \
		exit 1; \
	fi

.PHONY: deploy-spire-server
deploy-spire-server: cluster-up certs
	@$(ROOT_DIR)/scripts/spire-server/deploy.sh

.PHONY: undeploy-spire-server
undeploy-spire-server:
	@$(ROOT_DIR)/scripts/spire-server/undeploy.sh

.PHONY: check-spire-server
check-spire-server: check-cluster
	@$(ROOT_DIR)/scripts/check-spire-server.sh

.PHONY: deploy-spire-agent
deploy-spire-agent: certs
	@$(ROOT_DIR)/scripts/spire-agent/deploy.sh

.PHONY: undeploy-spire-agent
undeploy-spire-agent:
	@$(ROOT_DIR)/scripts/spire-agent/undeploy.sh

.PHONY: deploy-registration
deploy-registration: check-cluster
	@$(ROOT_DIR)/scripts/registration/deploy.sh

.PHONY: undeploy-registration
undeploy-registration:
	@$(ROOT_DIR)/scripts/registration/undeploy.sh

.PHONY: list-entries
list-entries: check-cluster
	@echo "[list-entries] Listing SPIRE workload entries..."
	@SPIRE_SERVER_POD=$$($(KUBECTL) get pods -n spire-server -l app=spire-server -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo ""); \
	if [ -z "$$SPIRE_SERVER_POD" ]; then \
		echo "[list-entries] Error: SPIRE server pod not found. Deploy SPIRE server first."; \
		exit 1; \
	fi; \
	$(KUBECTL) exec -n spire-server "$$SPIRE_SERVER_POD" -- \
		/opt/spire/bin/spire-server entry show || { \
		echo "[list-entries] Warning: Could not list entries"; \
		exit 1; \
	}

.PHONY: deploy-spire-csi
deploy-spire-csi: check-cluster
	@$(ROOT_DIR)/scripts/spire-csi/deploy.sh

.PHONY: undeploy-spire-csi
undeploy-spire-csi:
	@$(ROOT_DIR)/scripts/spire-csi/undeploy.sh

.PHONY: deploy-httpbin
deploy-httpbin: check-cluster
	@$(ROOT_DIR)/scripts/httpbin/deploy.sh

.PHONY: undeploy-httpbin
undeploy-httpbin:
	@$(ROOT_DIR)/scripts/httpbin/undeploy.sh

.PHONY: smoke-test
smoke-test: check-cluster
	@KUBECONFIG_PATH="$(KUBECONFIG_PATH)" ROOT_DIR="$(ROOT_DIR)" $(ROOT_DIR)/scripts/test/smoke-test.sh

# Top-level orchestration targets
.PHONY: env-up
env-up: tools certs cluster-up deploy-spire-server deploy-spire-agent deploy-spire-csi deploy-registration load-images deploy-httpbin
	@echo "[env-up] Environment setup complete!"

.PHONY: env-down
env-down: undeploy-httpbin undeploy-registration undeploy-spire-csi undeploy-spire-agent undeploy-spire-server cluster-down clean
	@echo "[env-down] Environment teardown complete!"

# Container image settings
HELPER_IMAGE_NAME ?= spiffe-helper
HELPER_IMAGE_TAG ?= test
DEBUG_IMAGE_NAME ?= spiffe-debug
DEBUG_IMAGE_TAG ?= latest

.PHONY: build-helper-image
build-helper-image:
	@echo "[build-helper-image] Building spiffe-helper container image..."
	@docker build -f spiffe-helper/Dockerfile -t "$(HELPER_IMAGE_NAME):$(HELPER_IMAGE_TAG)" .
	@echo "[build-helper-image] Helper container image built: $(HELPER_IMAGE_NAME):$(HELPER_IMAGE_TAG)"

.PHONY: load-helper-image
load-helper-image: check-cluster build-helper-image
	@echo "[load-helper-image] Loading spiffe-helper image into kind cluster..."
	@kind load docker-image "$(HELPER_IMAGE_NAME):$(HELPER_IMAGE_TAG)" --name "$(KIND_CLUSTER_NAME)"
	@echo "[load-helper-image] Helper image loaded into kind cluster"

.PHONY: build-debug-image
build-debug-image:
	@echo "[build-debug-image] Building debug container image..."
	@docker build -f "$(ROOT_DIR)/Dockerfile.debug" -t "$(DEBUG_IMAGE_NAME):$(DEBUG_IMAGE_TAG)" .
	@echo "[build-debug-image] Debug container image built: $(DEBUG_IMAGE_NAME):$(DEBUG_IMAGE_TAG)"

.PHONY: load-debug-image
load-debug-image: check-cluster build-debug-image
	@echo "[load-debug-image] Loading debug container image into kind cluster..."
	@kind load docker-image "$(DEBUG_IMAGE_NAME):$(DEBUG_IMAGE_TAG)" --name "$(KIND_CLUSTER_NAME)"
	@echo "[load-debug-image] Debug container image loaded into kind cluster"

.PHONY: load-images
load-images: load-helper-image load-debug-image
	@echo "[load-images] All images loaded into kind cluster"