ROOT_DIR := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
TOOLS_SCRIPT := $(ROOT_DIR)/scripts/install-tools.sh
CERT_SCRIPT := $(ROOT_DIR)/scripts/generate-certs.sh
KIND ?= kind
KIND_CLUSTER_NAME ?= spiffe-helper
KIND_CONFIG := $(ROOT_DIR)/kind-config.yaml
ARTIFACTS_DIR := $(ROOT_DIR)/artifacts
KUBECONFIG_PATH := $(ARTIFACTS_DIR)/kubeconfig
CERT_DIR := $(ARTIFACTS_DIR)/certs

.PHONY: tools
tools:
	@$(TOOLS_SCRIPT)

.PHONY: certs
certs:
	@$(CERT_SCRIPT)

.PHONY: clean
clean:
	@echo "[clean] Removing generated certificates..."
	@rm -rf $(CERT_DIR)
	@echo "[clean] Clean complete."

.PHONY: cluster-up
cluster-up: $(KIND_CONFIG)
	@mkdir -p "$(ARTIFACTS_DIR)"
	@if $(KIND) get clusters | grep -qx "$(KIND_CLUSTER_NAME)"; then \
		echo "kind cluster '$(KIND_CLUSTER_NAME)' already exists"; \
	else \
		echo "Creating kind cluster '$(KIND_CLUSTER_NAME)'"; \
		KUBECONFIG="$(KUBECONFIG_PATH)" $(KIND) create cluster --name "$(KIND_CLUSTER_NAME)" --config "$(KIND_CONFIG)"; \
	fi
	@$(KIND) get kubeconfig --name "$(KIND_CLUSTER_NAME)" > "$(KUBECONFIG_PATH)"
	@echo "Kubeconfig written to $(KUBECONFIG_PATH)"

.PHONY: cluster-down
cluster-down:
	@if $(KIND) get clusters | grep -qx "$(KIND_CLUSTER_NAME)"; then \
		echo "Deleting kind cluster '$(KIND_CLUSTER_NAME)'"; \
		$(KIND) delete cluster --name "$(KIND_CLUSTER_NAME)"; \
	else \
		echo "kind cluster '$(KIND_CLUSTER_NAME)' already absent"; \
	fi
	@rm -f "$(KUBECONFIG_PATH)"
	@if [ -d "$(ARTIFACTS_DIR)" ] && [ -z "$$(ls -A "$(ARTIFACTS_DIR)")" ]; then rmdir "$(ARTIFACTS_DIR)"; fi
