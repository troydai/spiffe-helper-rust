# Style Guide

This style guide defines coding standards for the spiffe-helper-rust project. These rules should be followed by both human developers and code agents (like Claude Code) when writing or modifying code.

## Access Control

### Struct and Enum Fields

**Rule: Struct and enum fields must maintain minimal access visibility.**

- Fields should be private (`field_name`) by default
- Use `pub` visibility only when external access is explicitly required
- Prefer providing public methods (getters/setters) over exposing fields directly
- This principle applies to both structs and enums

**Examples:**

```rust
// Good: Minimal access
struct Config {
    agent_address: String,  // private field
    cert_dir: PathBuf,      // private field
}

impl Config {
    pub fn agent_address(&self) -> &str { &self.agent_address }
    pub fn cert_dir(&self) -> &Path { self.cert_dir.as_path() }
}

// Avoid: Exposing fields directly unless necessary
struct Config {
    pub agent_address: String,  // public field - only if external access is required
    pub cert_dir: PathBuf,
}
```

## Code Organization

### Constants

**Rule: Constants must be declared at the beginning of the file, immediately after imports.**

- All `const` declarations should appear after `use` statements and before any function, struct, or enum definitions
- This improves code readability and makes constants easy to locate
- Group related constants together

**Example:**

```rust
use std::path::Path;
use anyhow::Result;

const UDS_PREFIX: &str = "unix://";
const DEFAULT_RETRY_ATTEMPTS: u32 = 10;
const MAX_BACKOFF_SECONDS: u64 = 16;

/// Function definitions follow...
pub async fn create_client() -> Result<()> {
    // ...
}
```

## References

- This style guide complements the project's `CLAUDE.md` file
- Follow Rust standard formatting conventions (`cargo fmt`)
- Follow Rust linting rules (`cargo clippy`)
