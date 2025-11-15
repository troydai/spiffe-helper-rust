ROOT_DIR := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
TOOLS_SCRIPT := $(ROOT_DIR)/scripts/install-tools.sh
CERT_SCRIPT := $(ROOT_DIR)/scripts/generate-certs.sh
DEPLOY_SPIRE_SCRIPT := $(ROOT_DIR)/scripts/deploy-spire-server.sh
UNDEPLOY_SPIRE_SCRIPT := $(ROOT_DIR)/scripts/undeploy-spire-server.sh
CHECK_SPIRE_SCRIPT := $(ROOT_DIR)/scripts/check-spire-server.sh
KIND ?= kind
KIND_CLUSTER_NAME ?= spiffe-helper
KIND_CONFIG := $(ROOT_DIR)/kind-config.yaml
ARTIFACTS_DIR := $(ROOT_DIR)/artifacts
KUBECONFIG_PATH := $(ARTIFACTS_DIR)/kubeconfig
CERT_DIR := $(ARTIFACTS_DIR)/certs
BOOTSTRAP_BUNDLE := $(CERT_DIR)/bootstrap-bundle.pem
SPIRE_AGENT_DIR := $(ROOT_DIR)/deploy/spire/agent
KUBECTL := KUBECONFIG="$(KUBECONFIG_PATH)" kubectl

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

# Check if cluster exists and kubeconfig is valid
.PHONY: check-cluster
check-cluster:
	@if ! $(KIND) get clusters | grep -qx "$(KIND_CLUSTER_NAME)"; then \
		echo "Error: kind cluster '$(KIND_CLUSTER_NAME)' does not exist. Run 'make cluster-up' first."; \
		exit 1; \
	fi
	@if [ ! -f "$(KUBECONFIG_PATH)" ]; then \
		echo "Error: kubeconfig not found at $(KUBECONFIG_PATH). Run 'make cluster-up' first."; \
		exit 1; \
	fi
	@if ! $(KUBECTL) cluster-info > /dev/null 2>&1; then \
		echo "Error: unable to connect to cluster. Run 'make cluster-up' first."; \
		exit 1; \
	fi

# Check if required certificate files exist
.PHONY: check-certs
check-certs:
	@if [ ! -f "$(CERT_DIR)/ca-cert.pem" ] || [ ! -f "$(CERT_DIR)/ca-key.pem" ]; then \
		echo "Error: CA certificate files not found. Expected: $(CERT_DIR)/ca-cert.pem, $(CERT_DIR)/ca-key.pem"; \
		echo "Run 'make certs' first."; \
		exit 1; \
	fi
	@if [ ! -f "$(CERT_DIR)/spire-server-cert.pem" ] || [ ! -f "$(CERT_DIR)/spire-server-key.pem" ]; then \
		echo "Error: SPIRE server certificate files not found. Expected: $(CERT_DIR)/spire-server-cert.pem, $(CERT_DIR)/spire-server-key.pem"; \
		echo "Run 'make certs' first."; \
		exit 1; \
	fi
	@if [ ! -f "$(CERT_DIR)/bootstrap-bundle.pem" ]; then \
		echo "Error: Bootstrap bundle not found. Expected: $(CERT_DIR)/bootstrap-bundle.pem"; \
		echo "Run 'make certs' first."; \
		exit 1; \
	fi

.PHONY: deploy-spire-server
deploy-spire-server: cluster-up certs
	@$(DEPLOY_SPIRE_SCRIPT)

.PHONY: undeploy-spire-server
undeploy-spire-server: check-cluster
	@$(UNDEPLOY_SPIRE_SCRIPT)

.PHONY: check-spire-server
check-spire-server: check-cluster
	@$(CHECK_SPIRE_SCRIPT)

.PHONY: deploy-spire-agent
deploy-spire-agent: certs
	@echo "[deploy-spire-agent] Deploying SPIRE agent..."
	@if [ ! -f "$(KUBECONFIG_PATH)" ]; then \
		echo "Error: Kubeconfig not found. Run 'make cluster-up' first."; \
		exit 1; \
	fi
	@if [ ! -f "$(BOOTSTRAP_BUNDLE)" ]; then \
		echo "Error: Bootstrap bundle not found at $(BOOTSTRAP_BUNDLE). Run 'make certs' first."; \
		exit 1; \
	fi
	@echo "[deploy-spire-agent] Creating namespace..."
	@$(KUBECTL) apply -f $(SPIRE_AGENT_DIR)/namespace.yaml
	@echo "[deploy-spire-agent] Creating bootstrap bundle Secret from $(BOOTSTRAP_BUNDLE)..."
	@$(KUBECTL) create secret generic spire-bundle -n spire-agent \
		--from-file=bundle.pem=$(BOOTSTRAP_BUNDLE) \
		--dry-run=client -o yaml | \
		$(KUBECTL) apply -f -
	@echo "[deploy-spire-agent] Applying SPIRE agent manifests..."
	@$(KUBECTL) apply -f $(SPIRE_AGENT_DIR)/serviceaccount.yaml
	@$(KUBECTL) apply -f $(SPIRE_AGENT_DIR)/clusterrole.yaml
	@$(KUBECTL) apply -f $(SPIRE_AGENT_DIR)/clusterrolebinding.yaml
	@$(KUBECTL) apply -f $(SPIRE_AGENT_DIR)/configmap.yaml
	@$(KUBECTL) apply -f $(SPIRE_AGENT_DIR)/daemonset.yaml
	@echo "[deploy-spire-agent] Waiting for SPIRE agent DaemonSet to be ready..."
	@timeout=300; \
	elapsed=0; \
	interval=5; \
	while [ $$elapsed -lt $$timeout ]; do \
		ready=$$($(KUBECTL) get daemonset spire-agent -n spire-agent -o jsonpath='{.status.numberReady}' 2>/dev/null || echo "0"); \
		desired=$$($(KUBECTL) get daemonset spire-agent -n spire-agent -o jsonpath='{.status.desiredNumberScheduled}' 2>/dev/null || echo "0"); \
		if [ "$$ready" = "$$desired" ] && [ "$$ready" != "0" ]; then \
			echo "[deploy-spire-agent] All $$ready/$$desired SPIRE agent pods are ready!"; \
			break; \
		fi; \
		echo "[deploy-spire-agent] Waiting... ($$ready/$$desired pods ready)"; \
		sleep $$interval; \
		elapsed=$$((elapsed + interval)); \
	done; \
	if [ $$elapsed -ge $$timeout ]; then \
		echo "[deploy-spire-agent] Warning: Timeout waiting for DaemonSet to be ready. Checking status..."; \
		$(KUBECTL) get daemonset spire-agent -n spire-agent; \
		$(KUBECTL) get pods -l app=spire-agent -n spire-agent; \
		exit 1; \
	fi
	@echo "[deploy-spire-agent] SPIRE agent DaemonSet status:"
	@$(KUBECTL) get daemonset spire-agent -n spire-agent
	@echo "[deploy-spire-agent] SPIRE agent pods:"
	@$(KUBECTL) get pods -l app=spire-agent -n spire-agent

.PHONY: undeploy-spire-agent
undeploy-spire-agent:
	@echo "[undeploy-spire-agent] Removing SPIRE agent..."
	@if [ ! -f "$(KUBECONFIG_PATH)" ]; then \
		echo "Kubeconfig not found. Skipping undeploy."; \
		exit 0; \
	fi
	@$(KUBECTL) delete daemonset spire-agent -n spire-agent --ignore-not-found=true
	@$(KUBECTL) delete configmap spire-agent-config -n spire-agent --ignore-not-found=true
	@$(KUBECTL) delete clusterrolebinding spire-agent-cluster-role-binding --ignore-not-found=true
	@$(KUBECTL) delete clusterrole spire-agent-cluster-role --ignore-not-found=true
	@$(KUBECTL) delete serviceaccount spire-agent -n spire-agent --ignore-not-found=true
	@$(KUBECTL) delete secret spire-bundle -n spire-agent --ignore-not-found=true
	@$(KUBECTL) delete namespace spire-agent --ignore-not-found=true
	@echo "[undeploy-spire-agent] SPIRE agent removed."

.PHONY: env-down
env-down: undeploy-spire-agent undeploy-spire-server cluster-down
	@echo "[env-down] Environment cleanup complete."
