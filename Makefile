ROOT_DIR := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
TOOLS_SCRIPT := $(ROOT_DIR)/scripts/install-tools.sh

.PHONY: tools
tools:
	@$(TOOLS_SCRIPT)
