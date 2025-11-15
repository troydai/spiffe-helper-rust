ROOT_DIR := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
TOOLS_SCRIPT := $(ROOT_DIR)/scripts/install-tools.sh
CERT_SCRIPT := $(ROOT_DIR)/scripts/generate-certs.sh
KIND ?= kind
KIND_CLUSTER_NAME ?= spiffe-helper
KIND_CONFIG := $(ROOT_DIR)/kind-config.yaml
ARTIFACTS_DIR := $(ROOT_DIR)/artifacts
KUBECONFIG_PATH := $(ARTIFACTS_DIR)/kubeconfig
CERT_DIR := $(ARTIFACTS_DIR)/certs
DEPLOY_DIR := $(ROOT_DIR)/deploy/spire/server
KUBECTL := KUBECONFIG="$(KUBECONFIG_PATH)" kubectl

# Certificate file paths
CA_CERT := $(CERT_DIR)/ca-cert.pem
CA_KEY := $(CERT_DIR)/ca-key.pem
SERVER_CERT := $(CERT_DIR)/spire-server-cert.pem
SERVER_KEY := $(CERT_DIR)/spire-server-key.pem
BOOTSTRAP_BUNDLE := $(CERT_DIR)/bootstrap-bundle.pem

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
	@if [ ! -f "$(CA_CERT)" ] || [ ! -f "$(CA_KEY)" ]; then \
		echo "Error: CA certificate files not found. Expected: $(CA_CERT), $(CA_KEY)"; \
		echo "Run 'make certs' first."; \
		exit 1; \
	fi
	@if [ ! -f "$(SERVER_CERT)" ] || [ ! -f "$(SERVER_KEY)" ]; then \
		echo "Error: SPIRE server certificate files not found. Expected: $(SERVER_CERT), $(SERVER_KEY)"; \
		echo "Run 'make certs' first."; \
		exit 1; \
	fi
	@if [ ! -f "$(BOOTSTRAP_BUNDLE)" ]; then \
		echo "Error: Bootstrap bundle not found. Expected: $(BOOTSTRAP_BUNDLE)"; \
		echo "Run 'make certs' first."; \
		exit 1; \
	fi

.PHONY: deploy-spire-server
deploy-spire-server: cluster-up certs
	@echo "[deploy] Deploying SPIRE server..."
	@echo "[deploy] Creating namespace..."
	@$(KUBECTL) apply -f $(DEPLOY_DIR)/namespace.yaml
	@echo "[deploy] Creating ServiceAccount..."
	@$(KUBECTL) apply -f $(DEPLOY_DIR)/serviceaccount.yaml
	@echo "[deploy] Creating Secrets from certificates..."
	@$(KUBECTL) create secret generic spire-server-tls \
		--from-file=server.crt=$(SERVER_CERT) \
		--from-file=server.key=$(SERVER_KEY) \
		--namespace=spire-server \
		--dry-run=client -o yaml | $(KUBECTL) apply -f -
	@$(KUBECTL) create secret generic spire-server-ca \
		--from-file=ca.crt=$(CA_CERT) \
		--from-file=ca.key=$(CA_KEY) \
		--namespace=spire-server \
		--dry-run=client -o yaml | $(KUBECTL) apply -f -
	@$(KUBECTL) create secret generic spire-server-bootstrap \
		--from-file=bundle.pem=$(BOOTSTRAP_BUNDLE) \
		--namespace=spire-server \
		--dry-run=client -o yaml | $(KUBECTL) apply -f -
	@echo "[deploy] Creating ConfigMap..."
	@$(KUBECTL) apply -f $(DEPLOY_DIR)/configmap.yaml
	@echo "[deploy] Creating Service..."
	@$(KUBECTL) apply -f $(DEPLOY_DIR)/service.yaml
	@echo "[deploy] Creating StatefulSet..."
	@$(KUBECTL) apply -f $(DEPLOY_DIR)/statefulset.yaml
	@echo "[deploy] Waiting for StatefulSet rollout..."
	@$(KUBECTL) rollout status statefulset/spire-server -n spire-server --timeout=300s
	@echo "[deploy] Waiting for pod to be ready..."
	@$(KUBECTL) wait --for=condition=ready pod -l app=spire-server -n spire-server --timeout=300s || \
		(echo "[deploy] Warning: Pod may not be fully ready. Check with: $(KUBECTL) get pods -n spire-server"; exit 1)
	@echo "[deploy] SPIRE server deployed successfully!"
	@echo "[deploy] Pod status:"
	@$(KUBECTL) get pods -n spire-server

.PHONY: undeploy-spire-server
undeploy-spire-server: check-cluster
	@echo "[undeploy] Removing SPIRE server..."
	@if $(KUBECTL) get namespace spire-server > /dev/null 2>&1; then \
		echo "[undeploy] Deleting StatefulSet..."; \
		$(KUBECTL) delete statefulset spire-server -n spire-server --ignore-not-found=true; \
		echo "[undeploy] Deleting Service..."; \
		$(KUBECTL) delete service spire-server -n spire-server --ignore-not-found=true; \
		echo "[undeploy] Deleting ConfigMap..."; \
		$(KUBECTL) delete configmap spire-server-config -n spire-server --ignore-not-found=true; \
		echo "[undeploy] Deleting Secrets..."; \
		$(KUBECTL) delete secret spire-server-tls spire-server-ca spire-server-bootstrap -n spire-server --ignore-not-found=true; \
		echo "[undeploy] Deleting ServiceAccount..."; \
		$(KUBECTL) delete serviceaccount spire-server -n spire-server --ignore-not-found=true; \
		echo "[undeploy] Deleting namespace..."; \
		$(KUBECTL) delete namespace spire-server --ignore-not-found=true; \
		echo "[undeploy] SPIRE server removed successfully!"; \
	else \
		echo "[undeploy] Namespace 'spire-server' does not exist. Nothing to remove."; \
	fi

.PHONY: check-spire-server
check-spire-server: check-cluster
	@echo "[check] Checking SPIRE server status..."
	@echo ""
	@echo "=== Pod Status ==="
	@$(KUBECTL) get pods -n spire-server -l app=spire-server || (echo "Error: SPIRE server namespace or pods not found. Run 'make deploy-spire-server' first."; exit 1)
	@echo ""
	@echo "=== Service Status ==="
	@$(KUBECTL) get svc -n spire-server spire-server || echo "Service not found"
	@echo ""
	@echo "=== Pod Logs (last 20 lines) ==="
	@$(KUBECTL) logs -n spire-server -l app=spire-server --tail=20 || echo "Unable to fetch logs"
	@echo ""
	@echo "=== Health Check ==="
	@if $(KUBECTL) get pod -n spire-server -l app=spire-server -o jsonpath='{.items[0].status.conditions[?(@.type=="Ready")].status}' | grep -q "True"; then \
		echo "✓ SPIRE server pod is Ready"; \
	else \
		echo "✗ SPIRE server pod is not Ready"; \
	fi
	@if $(KUBECTL) get pod -n spire-server -l app=spire-server -o jsonpath='{.items[0].status.containerStatuses[0].ready}' | grep -q "true"; then \
		echo "✓ SPIRE server container is ready"; \
	else \
		echo "✗ SPIRE server container is not ready"; \
	fi
	@echo ""
	@echo "To view full logs: $(KUBECTL) logs -n spire-server -l app=spire-server -f"
	@echo "To exec into pod: $(KUBECTL) exec -it -n spire-server -l app=spire-server -- /bin/sh"
