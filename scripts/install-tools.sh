#!/usr/bin/env bash
set -euo pipefail

info() {
  echo "[tools] $*"
}

error() {
  echo "[tools] $*" >&2
}

declare -A TOOL_DOCS=(
  [kind]="https://kind.sigs.k8s.io/docs/user/quick-start/#installation"
  [kubectl]="https://kubernetes.io/docs/tasks/tools/"
  [helm]="https://helm.sh/docs/intro/install/"
  [jq]="https://jqlang.github.io/jq/download/"
  [openssl]="https://www.openssl.org/source/"
)

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
      info "found ${name}: ${version_info}"
    else
      info "found ${name}"
    fi
    return 0
  fi

  error "missing ${name}. Install instructions: ${TOOL_DOCS[$name]}"
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
    error ""
    error "Install the missing tool(s) and re-run 'make tools'."
    exit 1
  fi

  info "all required tools are available."
}

main "$@"
