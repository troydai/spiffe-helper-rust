# Build stage
FROM rust:latest as builder

# Install protoc (required for spiffe crate)
RUN apt-get update && \
    apt-get install -y --no-install-recommends protobuf-compiler && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./

# Copy the actual source code
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install ca-certificates for TLS connections and dumb-init for signal handling
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates dumb-init && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/spiffe-helper-rust /usr/local/bin/spiffe-helper-rust

# Use dumb-init as entrypoint for proper signal handling
ENTRYPOINT ["dumb-init", "--"]
CMD ["/usr/local/bin/spiffe-helper-rust"]
