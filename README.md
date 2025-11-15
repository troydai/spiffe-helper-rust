# spiffe-helper-rust

A Rust implementation of spiffe-helper.

spiffe-helper fetches SPIFFE X.509 certificates and JWT tokens from the SPIRE agent. It acts as a bridge to integrate other programs with SPIRE.

## Tooling

This repository depends on several CLI tools (`kind`, `kubectl`, `helm`, `jq`, and `openssl`). We do **not** download or pin binary releases for you; instead, we provide a quick verification script so you can continue using whatever versions your package manager supplies.

### Verify prerequisites (`make tools`)

Run the following to confirm the required CLIs are present on your `PATH`:

```bash
make tools
```

The `tools` target runs `scripts/install-tools.sh`, which now **only checks** for the tools and prints their versions. If something is missing, the script exits with a failure code and echoes the upstream installation link so you can install/upgrade the tool yourself. Re-run `make tools` after installing to ensure everything is available.

### Installing the tools

Use whichever package manager you prefer. The examples below are common starting points; consult the official docs if you need additional options or platforms.

| Tool    | Homebrew (macOS)                   | Debian/Ubuntu (apt/snap/other)                 | Docs |
| ------- | ---------------------------------- | ---------------------------------------------- | ---- |
| kind    | `brew install kind`                | `GO111MODULE=on go install sigs.k8s.io/kind@latest` | https://kind.sigs.k8s.io/docs/user/quick-start/#installation |
| kubectl | `brew install kubectl`             | `sudo apt-get install -y kubectl`              | https://kubernetes.io/docs/tasks/tools/ |
| helm    | `brew install helm`                | `curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 \| bash` | https://helm.sh/docs/intro/install/ |
| jq      | `brew install jq`                  | `sudo apt-get install -y jq`                   | https://jqlang.github.io/jq/download/ |
| openssl | `brew install openssl`             | `sudo apt-get install -y openssl`              | https://www.openssl.org/source/ |

Feel free to substitute other installation methods (ASDF, Nix, direct downloads, etc.) as long as the resulting binaries land on your `PATH`. After installation, run `make tools` again to verify the environment.
