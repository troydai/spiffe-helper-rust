#!/bin/bash
# Validate Dockerfile changes for dumb-init integration

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

# Colors
source "$SCRIPT_DIR/colors.sh"

DOCKERFILE="$ROOT_DIR/Dockerfile"
ERRORS=0

echo -e "${COLOR_BLUE}[validate]${COLOR_RESET} Validating Dockerfile..."

# Check if Dockerfile exists
if [ ! -f "$DOCKERFILE" ]; then
    echo -e "${COLOR_RED}[validate]${COLOR_RESET} ERROR: Dockerfile not found"
    exit 1
fi

# Check 1: Verify dumb-init is installed
if grep -q "dumb-init" "$DOCKERFILE"; then
    echo -e "${COLOR_GREEN}[validate]${COLOR_RESET} ✓ dumb-init is installed"
else
    echo -e "${COLOR_RED}[validate]${COLOR_RESET} ERROR: dumb-init not found in Dockerfile"
    ERRORS=$((ERRORS + 1))
fi

# Check 2: Verify ENTRYPOINT uses dumb-init
if grep -q 'ENTRYPOINT \["dumb-init", "--"\]' "$DOCKERFILE"; then
    echo -e "${COLOR_GREEN}[validate]${COLOR_RESET} ✓ ENTRYPOINT uses dumb-init"
else
    echo -e "${COLOR_RED}[validate]${COLOR_RESET} ERROR: ENTRYPOINT does not use dumb-init"
    echo "  Expected: ENTRYPOINT [\"dumb-init\", \"--\"]"
    ERRORS=$((ERRORS + 1))
fi

# Check 3: Verify CMD is set
if grep -q 'CMD \["/usr/local/bin/spiffe-helper-rust"\]' "$DOCKERFILE"; then
    echo -e "${COLOR_GREEN}[validate]${COLOR_RESET} ✓ CMD is set correctly"
else
    echo -e "${COLOR_RED}[validate]${COLOR_RESET} ERROR: CMD not set correctly"
    echo "  Expected: CMD [\"/usr/local/bin/spiffe-helper-rust\"]"
    ERRORS=$((ERRORS + 1))
fi

# Check 4: Verify dumb-init is in the same RUN command as ca-certificates (efficiency)
if grep -A 2 "apt-get install" "$DOCKERFILE" | grep -q "dumb-init"; then
    echo -e "${COLOR_GREEN}[validate]${COLOR_RESET} ✓ dumb-init installed efficiently with ca-certificates"
else
    echo -e "${COLOR_YELLOW}[validate]${COLOR_RESET} WARNING: dumb-init may not be installed efficiently"
fi

# Check 5: Verify no old ENTRYPOINT pattern exists
if grep -q 'ENTRYPOINT \["/usr/local/bin/spiffe-helper-rust"\]' "$DOCKERFILE"; then
    echo -e "${COLOR_RED}[validate]${COLOR_RESET} ERROR: Old ENTRYPOINT pattern still exists"
    ERRORS=$((ERRORS + 1))
else
    echo -e "${COLOR_GREEN}[validate]${COLOR_RESET} ✓ Old ENTRYPOINT pattern removed"
fi

# Summary
echo ""
if [ $ERRORS -eq 0 ]; then
    echo -e "${COLOR_BRIGHT_GREEN}[validate]${COLOR_RESET} All validations passed!"
    exit 0
else
    echo -e "${COLOR_RED}[validate]${COLOR_RESET} Found $ERRORS error(s)"
    exit 1
fi
