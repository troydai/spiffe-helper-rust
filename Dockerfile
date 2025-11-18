# Build stage
FROM rust:1.75-slim as builder

WORKDIR /app

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy source file to build dependencies
# This allows Docker to cache the dependency build layer
RUN mkdir -p src && \
    echo "mod config;" > src/main.rs && \
    echo "fn main() {}" >> src/main.rs && \
    echo "// dummy config module" > src/config.rs && \
    cargo build --release && \
    rm -rf src

# Copy the actual source code
COPY src ./src

# Build the actual application
# Touch main.rs to ensure cargo rebuilds the binary
RUN touch src/main.rs && \
    cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install ca-certificates for TLS connections
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/spiffe-helper-rust /usr/local/bin/spiffe-helper-rust

# Set the entrypoint
ENTRYPOINT ["/usr/local/bin/spiffe-helper-rust"]
