## Additional Context

### Why This Matters

- The certificate chain handling is a critical security feature for proper TLS validation
- Future configuration options (see IMPLEMENTATION_PLAN.md Phase 4.1) will make intermediate certificate handling configurable
- Clear documentation will help developers understand the default behavior vs. future optional behaviors

### Code References

- **Current location**: `/home/user/spiffe-helper-rust/src/workload_api.rs:96-107`
- **Related test**: `/home/user/spiffe-helper-rust/scripts/test-oneshot-x509.sh:245-246` (validates certificate with openssl)
- **Future work**: `/home/user/spiffe-helper-rust/IMPLEMENTATION_PLAN.md:175-178` (add_intermediates_to_bundle option)

### Current Code (Needs Comment)

```rust
// Write certificate (PEM format)
let cert_pem = svid
    .cert_chain()
    .iter()
    .map(|cert| {
        pem::encode(&pem::Pem {
            tag: "CERTIFICATE".to_string(),
            contents: cert.as_ref().to_vec(),
        })
    })
    .collect::<Vec<_>>()
    .join("\n");

fs::write(&cert_path, cert_pem)
    .with_context(|| format!("Failed to write certificate to {}", cert_path.display()))?;
```

### Implementation Notes

The comment should explain:

1. **What**: That `cert_chain()` returns the full chain (leaf + intermediates, potentially + root)
2. **Why**: Including intermediates is necessary - enables TLS clients to validate the certificate without requiring pre-installed intermediate CAs
3. **Format**: The PEM format - multiple certificates concatenated with newlines in a single file (industry standard)
4. **Ordering**: Typically leaf certificate first, followed by intermediates in chain order

### Example Comment Structure

```rust
// Write the complete X.509 certificate chain to a single PEM file.
// The cert_chain() includes the leaf certificate and any intermediate certificates
// required for certificate validation. Including intermediates is critical because
// TLS/SSL clients need the complete chain to validate the certificate against their
// trusted root CAs. All certificates are encoded as PEM blocks and concatenated with
// newlines, which is the standard format for certificate chain files.
```

### Good First Issue

This is an excellent first issue for someone learning the codebase because:
- It requires understanding the SPIFFE/SPIRE certificate model
- No code changes needed, only documentation
- Low risk, high educational value
- Helps future contributors understand a critical security feature
