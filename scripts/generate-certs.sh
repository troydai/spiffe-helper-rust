#!/usr/bin/env bash
# Generates CA and SPIRE server certificates for the local development environment.
set -euo pipefail

# Source color support
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/colors.sh"

info() {
  echo -e "${COLOR_CYAN}[certs]${COLOR_RESET} $*"
}

error() {
  echo -e "${COLOR_RED}[certs]${COLOR_RESET} $*" >&2
}

# Default values
CERT_DIR="${CERT_DIR:-./artifacts/certs}"
FORCE="${FORCE:-0}"

# Certificate configuration
CA_KEY="${CERT_DIR}/ca-key.pem"
CA_CERT="${CERT_DIR}/ca-cert.pem"
SERVER_KEY="${CERT_DIR}/spire-server-key.pem"
SERVER_CERT="${CERT_DIR}/spire-server-cert.pem"
SERVER_CSR="${CERT_DIR}/spire-server.csr"
BOOTSTRAP_BUNDLE="${CERT_DIR}/bootstrap-bundle.pem"

# Create certs directory if it doesn't exist
mkdir -p "${CERT_DIR}"

# Check if openssl is available
if ! command -v openssl >/dev/null 2>&1; then
  error "openssl is required but not found. Install it and re-run."
  exit 1
fi

# Function to check if a file exists and is non-empty
file_exists() {
  [[ -f "$1" ]] && [[ -s "$1" ]]
}

# Function to generate CA certificate and key
generate_ca() {
  if [[ "${FORCE}" -eq 0 ]] && file_exists "${CA_KEY}" && file_exists "${CA_CERT}"; then
    echo -e "${COLOR_YELLOW}[certs]${COLOR_RESET} CA certificate and key already exist, skipping generation"
    return 0
  fi

  echo -e "${COLOR_CYAN}[certs]${COLOR_RESET} Generating CA private key ${COLOR_BOLD}(ECDSA P-384)${COLOR_RESET} in PKCS#8 format..."
  openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:secp384r1 -out "${CA_KEY}"

  echo -e "${COLOR_CYAN}[certs]${COLOR_RESET} Generating CA certificate..."
  openssl req -new -x509 -days 3650 -key "${CA_KEY}" -out "${CA_CERT}" \
    -subj "/CN=spiffe-helper-sandbox-ca" \
    -addext "basicConstraints=critical,CA:TRUE" \
    -addext "keyUsage=critical,keyCertSign,cRLSign"

  echo -e "${COLOR_GREEN}✓${COLOR_RESET} CA certificate and key generated successfully"
}

# Function to generate SPIRE server certificate and key
generate_spire_server() {
  if [[ "${FORCE}" -eq 0 ]] && file_exists "${SERVER_KEY}" && file_exists "${SERVER_CERT}"; then
    echo -e "${COLOR_YELLOW}[certs]${COLOR_RESET} SPIRE server certificate and key already exist, skipping generation"
    return 0
  fi

  echo -e "${COLOR_CYAN}[certs]${COLOR_RESET} Generating SPIRE server private key ${COLOR_BOLD}(ECDSA P-256)${COLOR_RESET} in PKCS#8 format..."
  openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:prime256v1 -out "${SERVER_KEY}"

  echo -e "${COLOR_CYAN}[certs]${COLOR_RESET} Generating SPIRE server certificate signing request..."
  openssl req -new -key "${SERVER_KEY}" -out "${SERVER_CSR}" \
    -subj "/CN=spiffe-helper-sandbox-spire-server"

  echo -e "${COLOR_CYAN}[certs]${COLOR_RESET} Generating SPIRE server certificate ${COLOR_BOLD}(signed by CA)${COLOR_RESET}..."
  openssl x509 -req -in "${SERVER_CSR}" -CA "${CA_CERT}" -CAkey "${CA_KEY}" \
    -CAcreateserial -out "${SERVER_CERT}" -days 365 \
    -extensions v3_server -extfile <(
      echo "[v3_server]"
      echo "basicConstraints=CA:FALSE"
      echo "keyUsage=digitalSignature"
      echo "extendedKeyUsage=serverAuth"
      echo "subjectAltName=DNS:spire-server,DNS:spire-server.default.svc.cluster.local"
    )

  echo -e "${COLOR_GREEN}✓${COLOR_RESET} SPIRE server certificate and key generated successfully"
}

# Function to generate bootstrap bundle
generate_bootstrap_bundle() {
  if [[ "${FORCE}" -eq 0 ]] && file_exists "${BOOTSTRAP_BUNDLE}"; then
    echo -e "${COLOR_YELLOW}[certs]${COLOR_RESET} Bootstrap bundle already exists, skipping generation"
    return 0
  fi

  if ! file_exists "${CA_CERT}"; then
    error "CA certificate not found. Cannot generate bootstrap bundle."
    exit 1
  fi

  echo -e "${COLOR_CYAN}[certs]${COLOR_RESET} Generating bootstrap bundle..."
  cp "${CA_CERT}" "${BOOTSTRAP_BUNDLE}"

  echo -e "${COLOR_GREEN}✓${COLOR_RESET} Bootstrap bundle generated successfully"
}

main() {
  echo -e "${COLOR_BRIGHT_BLUE}[certs]${COLOR_RESET} ${COLOR_BOLD}Starting certificate generation...${COLOR_RESET}"
  echo -e "${COLOR_CYAN}[certs]${COLOR_RESET} Output directory: ${COLOR_BOLD}${CERT_DIR}${COLOR_RESET}"

  generate_ca
  generate_spire_server
  generate_bootstrap_bundle

  echo ""
  echo -e "${COLOR_BRIGHT_GREEN}[certs]${COLOR_RESET} ${COLOR_BOLD}Certificate generation complete!${COLOR_RESET}"
  echo ""
  echo -e "${COLOR_CYAN}[certs]${COLOR_RESET} Generated files:"
  echo -e "  ${COLOR_GREEN}✓${COLOR_RESET} CA: ${COLOR_CYAN}${CA_KEY}${COLOR_RESET}, ${COLOR_CYAN}${CA_CERT}${COLOR_RESET}"
  echo -e "  ${COLOR_GREEN}✓${COLOR_RESET} SPIRE Server: ${COLOR_CYAN}${SERVER_KEY}${COLOR_RESET}, ${COLOR_CYAN}${SERVER_CERT}${COLOR_RESET}, ${COLOR_CYAN}${SERVER_CSR}${COLOR_RESET}"
  echo -e "  ${COLOR_GREEN}✓${COLOR_RESET} Bootstrap Bundle: ${COLOR_CYAN}${BOOTSTRAP_BUNDLE}${COLOR_RESET}"
}

main "$@"

