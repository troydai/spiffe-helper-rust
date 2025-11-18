FROM alpine:latest

# Install ca-certificates for HTTPS requests
RUN apk add --no-cache ca-certificates

# Copy the binary
COPY spiffe-helper-rust /usr/local/bin/spiffe-helper-rust
RUN chmod +x /usr/local/bin/spiffe-helper-rust

# Create directory for certs
RUN mkdir -p /tmp/certs

# Run the daemon
ENTRYPOINT ["/usr/local/bin/spiffe-helper-rust"]
