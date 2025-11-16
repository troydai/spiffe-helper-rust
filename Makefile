ROOT_DIR := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
TOOLS_SCRIPT := $(ROOT_DIR)/scripts/install-tools.sh
CERT_SCRIPT := $(ROOT_DIR)/scripts/generate-certs.sh
CLUSTER_UP_SCRIPT := $(ROOT_DIR)/scripts/cluster-up.sh
DEPLOY_SPIRE_SCRIPT := $(ROOT_DIR)/scripts/deploy-spire-server.sh
UNDEPLOY_SPIRE_SCRIPT := $(ROOT_DIR)/scripts/undeploy-spire-server.sh
CHECK_SPIRE_SCRIPT := $(ROOT_DIR)/scripts/check-spire-server.sh
DEPLOY_SPIRE_AGENT_SCRIPT := $(ROOT_DIR)/scripts/deploy-spire-agent.sh
UNDEPLOY_SPIRE_AGENT_SCRIPT := $(ROOT_DIR)/scripts/undeploy-spire-agent.sh
DEPLOY_REGISTRATION_SCRIPT := $(ROOT_DIR)/scripts/deploy-registration.sh
UNDEPLOY_REGISTRATION_SCRIPT := $(ROOT_DIR)/scripts/undeploy-registration.sh
KIND ?= kind
KIND_CLUSTER_NAME ?= spiffe-helper
KIND_CONFIG := $(ROOT_DIR)/kind-config.yaml
ARTIFACTS_DIR := $(ROOT_DIR)/artifacts
KUBECONFIG_PATH := $(ARTIFACTS_DIR)/kubeconfig
CERT_DIR := $(ARTIFACTS_DIR)/certs
BOOTSTRAP_BUNDLE := $(CERT_DIR)/bootstrap-bundle.pem
SPIRE_AGENT_DIR := $(ROOT_DIR)/deploy/spire/agent
KUBECTL := KUBECONFIG="$(KUBECONFIG_PATH)" kubectl

# Color definitions (only if output is a TTY)
ifneq ($(shell [ -t 1 ] && echo yes),)
  COLOR_RESET := \033[0m
  COLOR_BOLD := \033[1m
  COLOR_RED := \033[0;31m
  COLOR_GREEN := \033[0;32m
  COLOR_YELLOW := \033[0;33m
  COLOR_BLUE := \033[0;34m
  COLOR_CYAN := \033[0;36m
  COLOR_BRIGHT_GREEN := \033[1;32m
  COLOR_BRIGHT_BLUE := \033[1;34m
else
  COLOR_RESET :=
  COLOR_BOLD :=
  COLOR_RED :=
  COLOR_GREEN :=
  COLOR_YELLOW :=
  COLOR_BLUE :=
  COLOR_CYAN :=
  COLOR_BRIGHT_GREEN :=
  COLOR_BRIGHT_BLUE :=
endif

.PHONY: tools
tools:
	@$(TOOLS_SCRIPT)

.PHONY: certs
certs:
	@$(CERT_SCRIPT)

.PHONY: clean
clean:
	@echo "$(COLOR_CYAN)[clean]$(COLOR_RESET) Removing generated artifacts..."
	@rm -rf $(ARTIFACTS_DIR)
	@if [ -d "$(ROOT_DIR)/bin" ]; then \
		echo "$(COLOR_CYAN)[clean]$(COLOR_RESET) Removing binaries..."; \
		rm -rf "$(ROOT_DIR)/bin"; \
	fi
	@echo "$(COLOR_GREEN)[clean]$(COLOR_RESET) Clean complete."

.PHONY: cluster-up
cluster-up: $(KIND_CONFIG)
	@KIND="$(KIND)" KIND_CLUSTER_NAME="$(KIND_CLUSTER_NAME)" KIND_CONFIG="$(KIND_CONFIG)" ARTIFACTS_DIR="$(ARTIFACTS_DIR)" KUBECONFIG_PATH="$(KUBECONFIG_PATH)" ROOT_DIR="$(ROOT_DIR)" $(CLUSTER_UP_SCRIPT)

.PHONY: cluster-down
cluster-down:
	@if $(KIND) get clusters | grep -qx "$(KIND_CLUSTER_NAME)"; then \
		echo "$(COLOR_CYAN)[cluster-down]$(COLOR_RESET) Deleting kind cluster '$(COLOR_BOLD)$(KIND_CLUSTER_NAME)$(COLOR_RESET)'"; \
		$(KIND) delete cluster --name "$(KIND_CLUSTER_NAME)"; \
	else \
		echo "$(COLOR_YELLOW)[cluster-down]$(COLOR_RESET) kind cluster '$(COLOR_BOLD)$(KIND_CLUSTER_NAME)$(COLOR_RESET)' already absent"; \
	fi
	@rm -f "$(KUBECONFIG_PATH)"
	@if [ -d "$(ARTIFACTS_DIR)" ] && [ -z "$$(ls -A "$(ARTIFACTS_DIR)")" ]; then rmdir "$(ARTIFACTS_DIR)"; fi

# Check if cluster exists and kubeconfig is valid
.PHONY: check-cluster
check-cluster:
	@if ! $(KIND) get clusters | grep -qx "$(KIND_CLUSTER_NAME)"; then \
		echo "$(COLOR_RED)[check-cluster] Error:$(COLOR_RESET) kind cluster '$(COLOR_BOLD)$(KIND_CLUSTER_NAME)$(COLOR_RESET)' does not exist. Run 'make cluster-up' first."; \
		exit 1; \
	fi
	@if [ ! -f "$(KUBECONFIG_PATH)" ]; then \
		echo "$(COLOR_RED)[check-cluster] Error:$(COLOR_RESET) kubeconfig not found at $(COLOR_CYAN)$(KUBECONFIG_PATH)$(COLOR_RESET). Run 'make cluster-up' first."; \
		exit 1; \
	fi
	@if ! $(KUBECTL) cluster-info > /dev/null 2>&1; then \
		echo "$(COLOR_RED)[check-cluster] Error:$(COLOR_RESET) unable to connect to cluster. Run 'make cluster-up' first."; \
		exit 1; \
	fi

# Check if required certificate files exist
.PHONY: check-certs
check-certs:
	@if [ ! -f "$(CERT_DIR)/ca-cert.pem" ] || [ ! -f "$(CERT_DIR)/ca-key.pem" ]; then \
		echo "$(COLOR_RED)[check-certs] Error:$(COLOR_RESET) CA certificate files not found. Expected: $(COLOR_CYAN)$(CERT_DIR)/ca-cert.pem$(COLOR_RESET), $(COLOR_CYAN)$(CERT_DIR)/ca-key.pem$(COLOR_RESET)"; \
		echo "$(COLOR_YELLOW)[check-certs]$(COLOR_RESET) Run 'make certs' first."; \
		exit 1; \
	fi
	@if [ ! -f "$(CERT_DIR)/spire-server-cert.pem" ] || [ ! -f "$(CERT_DIR)/spire-server-key.pem" ]; then \
		echo "$(COLOR_RED)[check-certs] Error:$(COLOR_RESET) SPIRE server certificate files not found. Expected: $(COLOR_CYAN)$(CERT_DIR)/spire-server-cert.pem$(COLOR_RESET), $(COLOR_CYAN)$(CERT_DIR)/spire-server-key.pem$(COLOR_RESET)"; \
		echo "$(COLOR_YELLOW)[check-certs]$(COLOR_RESET) Run 'make certs' first."; \
		exit 1; \
	fi
	@if [ ! -f "$(CERT_DIR)/bootstrap-bundle.pem" ]; then \
		echo "$(COLOR_RED)[check-certs] Error:$(COLOR_RESET) Bootstrap bundle not found. Expected: $(COLOR_CYAN)$(CERT_DIR)/bootstrap-bundle.pem$(COLOR_RESET)"; \
		echo "$(COLOR_YELLOW)[check-certs]$(COLOR_RESET) Run 'make certs' first."; \
		exit 1; \
	fi

.PHONY: deploy-spire-server
deploy-spire-server: cluster-up certs
	@$(DEPLOY_SPIRE_SCRIPT)

.PHONY: undeploy-spire-server
undeploy-spire-server:
	@$(UNDEPLOY_SPIRE_SCRIPT)

.PHONY: check-spire-server
check-spire-server: check-cluster
	@$(CHECK_SPIRE_SCRIPT)

.PHONY: deploy-spire-agent
deploy-spire-agent: certs
	@$(DEPLOY_SPIRE_AGENT_SCRIPT)

.PHONY: undeploy-spire-agent
undeploy-spire-agent:
	@$(UNDEPLOY_SPIRE_AGENT_SCRIPT)

.PHONY: deploy-registration
deploy-registration: check-cluster
	@$(DEPLOY_REGISTRATION_SCRIPT)

.PHONY: undeploy-registration
undeploy-registration:
	@$(UNDEPLOY_REGISTRATION_SCRIPT)

# Top-level orchestration targets
.PHONY: env-up
env-up: tools certs cluster-up deploy-spire-server deploy-spire-agent deploy-registration
	@echo "$(COLOR_BRIGHT_GREEN)[env-up]$(COLOR_RESET) $(COLOR_BOLD)Environment setup complete!$(COLOR_RESET)"

.PHONY: env-down
env-down: undeploy-registration undeploy-spire-agent undeploy-spire-server cluster-down clean
	@echo "$(COLOR_BRIGHT_GREEN)[env-down]$(COLOR_RESET) $(COLOR_BOLD)Environment teardown complete!$(COLOR_RESET)"
