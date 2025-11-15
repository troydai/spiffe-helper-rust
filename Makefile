ROOT_DIR := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
TOOLS_SCRIPT := $(ROOT_DIR)/scripts/install-tools.sh
CERT_SCRIPT := $(ROOT_DIR)/scripts/generate-certs.sh
CERT_DIR := $(ROOT_DIR)/artifacts/certs

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
