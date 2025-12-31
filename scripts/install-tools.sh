#!/usr/bin/env bash
# Checks for and provides installation instructions for required development tools like kind, kubectl, and openssl.
set -euo pipefail

# Source color support
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/utility/colors.sh"

info() {
  echo -e "${COLOR_CYAN}[tools]${COLOR_RESET} $*"
}

error() {
  echo -e "${COLOR_RED}[tools]${COLOR_RESET} $*" >&2
}

get_tool_docs() {
  local name="$1"
  case "${name}" in
    kind)
      echo "https://kind.sigs.k8s.io/docs/user/quick-start/#installation"
      ;;
    kubectl)
      echo "https://kubernetes.io/docs/tasks/tools/"
      ;;
    helm)
      echo "https://helm.sh/docs/intro/install/"
      ;;
    jq)
      echo "https://jqlang.github.io/jq/download/"
      ;;
    openssl)
      echo "https://www.openssl.org/source/"
      ;;
    *)
      echo ""
      ;;
  esac
}

print_version() {
  local name="$1"
  local output
  case "${name}" in
    kind)
      output="$(kind version 2>&1)" || return 1
      ;;
    kubectl)
      if output="$(kubectl version --client --short 2>&1)"; then
        :
      else
        output="$(kubectl version --client 2>&1)" || return 1
      fi
      ;;
    helm)
      output="$(helm version --short 2>&1)" || return 1
      ;;
    jq)
      output="$(jq --version 2>&1)" || return 1
      ;;
    openssl)
      output="$(openssl version 2>&1)" || return 1
      ;;
    *)
      return 1
      ;;
  esac

  printf "%s\n" "${output}" | head -n1
}

check_tool() {
  local name="$1"
  if command -v "${name}" >/dev/null 2>&1; then
    local version_info
    if version_info="$(print_version "${name}")"; then
      echo -e "${COLOR_GREEN}✓${COLOR_RESET} ${COLOR_BOLD}${name}${COLOR_RESET}: ${COLOR_CYAN}${version_info}${COLOR_RESET}"
    else
      echo -e "${COLOR_GREEN}✓${COLOR_RESET} ${COLOR_BOLD}${name}${COLOR_RESET}"
    fi
    return 0
  fi

  local docs_url
  docs_url="$(get_tool_docs "${name}")"
  echo -e "${COLOR_RED}✗${COLOR_RESET} ${COLOR_BOLD}${name}${COLOR_RESET} ${COLOR_RED}missing${COLOR_RESET}"
  echo -e "  ${COLOR_YELLOW}Install instructions:${COLOR_RESET} ${COLOR_CYAN}${docs_url}${COLOR_RESET}" >&2
  return 1
}

main() {
  local missing=0
  for tool in kind kubectl helm jq openssl; do
    if ! check_tool "${tool}"; then
      missing=1
    fi
  done

  if [[ "${missing}" -eq 1 ]]; then
    echo ""
    error "Install the missing tool(s) and re-run 'make tools'."
    exit 1
  fi

  echo ""
  echo -e "${COLOR_BRIGHT_GREEN}[tools]${COLOR_RESET} ${COLOR_BOLD}All required tools are available!${COLOR_RESET}"
}

main "$@"
