#!/usr/bin/env bash
set -euo pipefail

info() {
  echo "[certs] $*"
}

error() {
  echo "[certs] $*" >&2
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
JWT_KEY="${CERT_DIR}/jwt-signing-key.pem"
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
    info "CA certificate and key already exist, skipping generation"
    return 0
  fi

  info "Generating CA private key (ECDSA P-384)..."
  openssl ecparam -genkey -name secp384r1 -out "${CA_KEY}"

  info "Generating CA certificate..."
  openssl req -new -x509 -days 3650 -key "${CA_KEY}" -out "${CA_CERT}" \
    -subj "/CN=spiffe-helper-sandbox-ca" \
    -addext "basicConstraints=critical,CA:TRUE" \
    -addext "keyUsage=critical,keyCertSign,cRLSign"

  info "CA certificate and key generated successfully"
}

# Function to generate SPIRE server certificate and key
generate_spire_server() {
  if [[ "${FORCE}" -eq 0 ]] && file_exists "${SERVER_KEY}" && file_exists "${SERVER_CERT}"; then
    info "SPIRE server certificate and key already exist, skipping generation"
    return 0
  fi

  info "Generating SPIRE server private key (ECDSA P-256)..."
  openssl ecparam -genkey -name secp256r1 -out "${SERVER_KEY}"

  info "Generating SPIRE server certificate signing request..."
  openssl req -new -key "${SERVER_KEY}" -out "${SERVER_CSR}" \
    -subj "/CN=spiffe-helper-sandbox-spire-server"

  info "Generating SPIRE server certificate (signed by CA)..."
  openssl x509 -req -in "${SERVER_CSR}" -CA "${CA_CERT}" -CAkey "${CA_KEY}" \
    -CAcreateserial -out "${SERVER_CERT}" -days 365 \
    -extensions v3_server -extfile <(
      echo "[v3_server]"
      echo "basicConstraints=CA:FALSE"
      echo "keyUsage=digitalSignature"
      echo "extendedKeyUsage=serverAuth"
      echo "subjectAltName=DNS:spire-server,DNS:spire-server.default.svc.cluster.local"
    )

  info "SPIRE server certificate and key generated successfully"
}

# Function to generate JWT signing key
generate_jwt_key() {
  if [[ "${FORCE}" -eq 0 ]] && file_exists "${JWT_KEY}"; then
    info "JWT signing key already exists, skipping generation"
    return 0
  fi

  info "Generating JWT signing key (ECDSA P-256)..."
  openssl ecparam -genkey -name secp256r1 -out "${JWT_KEY}"

  info "JWT signing key generated successfully"
}

# Function to generate bootstrap bundle
generate_bootstrap_bundle() {
  if [[ "${FORCE}" -eq 0 ]] && file_exists "${BOOTSTRAP_BUNDLE}"; then
    info "Bootstrap bundle already exists, skipping generation"
    return 0
  fi

  if ! file_exists "${CA_CERT}"; then
    error "CA certificate not found. Cannot generate bootstrap bundle."
    exit 1
  fi

  info "Generating bootstrap bundle..."
  cp "${CA_CERT}" "${BOOTSTRAP_BUNDLE}"

  info "Bootstrap bundle generated successfully"
}

main() {
  info "Starting certificate generation..."
  info "Output directory: ${CERT_DIR}"

  generate_ca
  generate_spire_server
  generate_jwt_key
  generate_bootstrap_bundle

  info "Certificate generation complete!"
  info ""
  info "Generated files:"
  info "  - CA: ${CA_KEY}, ${CA_CERT}"
  info "  - SPIRE Server: ${SERVER_KEY}, ${SERVER_CERT}, ${SERVER_CSR}"
  info "  - JWT Signing: ${JWT_KEY}"
  info "  - Bootstrap Bundle: ${BOOTSTRAP_BUNDLE}"
}

main "$@"

