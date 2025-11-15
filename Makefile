ROOT_DIR := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
TOOLS_SCRIPT := $(ROOT_DIR)/scripts/install-tools.sh
KIND ?= kind
KIND_CLUSTER_NAME ?= spiffe-helper
KIND_CONFIG_TEMPLATE := $(ROOT_DIR)/kind-config.yaml
ARTIFACTS_DIR := $(ROOT_DIR)/artifacts
KIND_RENDERED_CONFIG := $(ARTIFACTS_DIR)/kind-config.rendered.yaml
KUBECONFIG_PATH := $(ARTIFACTS_DIR)/kubeconfig
CERTS_DIR ?= $(ROOT_DIR)/certs

.PHONY: tools
tools:
	@$(TOOLS_SCRIPT)

.PHONY: cluster-up
cluster-up: $(KIND_CONFIG_TEMPLATE)
	@mkdir -p "$(ARTIFACTS_DIR)"
	@mkdir -p "$(CERTS_DIR)"
	@sed "s|\$${CERTS_DIR}|$(CERTS_DIR)|g" "$(KIND_CONFIG_TEMPLATE)" > "$(KIND_RENDERED_CONFIG)"
	@if $(KIND) get clusters | grep -qx "$(KIND_CLUSTER_NAME)"; then \
		echo "kind cluster '$(KIND_CLUSTER_NAME)' already exists"; \
	else \
		echo "Creating kind cluster '$(KIND_CLUSTER_NAME)'"; \
		KUBECONFIG="$(KUBECONFIG_PATH)" $(KIND) create cluster --name "$(KIND_CLUSTER_NAME)" --config "$(KIND_RENDERED_CONFIG)"; \
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
	@rm -f "$(KUBECONFIG_PATH)" "$(KIND_RENDERED_CONFIG)"
	@if [ -d "$(ARTIFACTS_DIR)" ] && [ -z "$$(ls -A "$(ARTIFACTS_DIR)")" ]; then rmdir "$(ARTIFACTS_DIR)"; fi
