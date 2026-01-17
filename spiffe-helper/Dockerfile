# Build stage
FROM rust:bookworm as builder

# Install protoc (required for spiffe crate)
RUN apt-get update && \
    apt-get install -y --no-install-recommends protobuf-compiler && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY spiffe-helper/Cargo.toml ./spiffe-helper/Cargo.toml
COPY spire-agent-mock/Cargo.toml ./spire-agent-mock/Cargo.toml

# Copy the actual source code
COPY spiffe-helper/src ./spiffe-helper/src
COPY spire-agent-mock/src ./spire-agent-mock/src

# Build the application
RUN cargo build --release -p spiffe-helper

# Runtime stage
FROM debian:bookworm-slim

# Install ca-certificates for TLS connections and dumb-init for signal handling
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates dumb-init openssl && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/spiffe-helper /usr/local/bin/spiffe-helper

# Use dumb-init as entrypoint for proper signal handling
ENTRYPOINT ["dumb-init", "--"]
CMD ["/usr/local/bin/spiffe-helper"]
