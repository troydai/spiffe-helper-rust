# spiffe-helper-rust Feature Gap Review (vs upstream spiffe-helper README)

This document reviews the features implemented on the **main** branch of this repo and compares them with the feature set documented in the upstream `spiffe-helper` README:
<https://github.com/spiffe/spiffe-helper/blob/main/README.md>.

## Scope

* **Target implementation:** `spiffe-helper-rust` main branch (code under `spiffe-helper/` and repo root documentation).
* **Reference feature list:** upstream `spiffe-helper` README.

## Summary of observed implementation capabilities

Based on the current Rust implementation:

* **X.509 SVID fetching/writing** is implemented for both daemon and one-shot mode.
* **Trust bundle writing** for the SVID’s trust domain is implemented.
* **Managed child process support** (`cmd`, `cmd_args`) with signal on renewal is implemented.
* **PID-file signaling** on renewal is implemented.
* **Health check server** exists (liveness/readiness endpoints always return 200).
* **Configuration parsing** supports many options (e.g., JWT SVIDs, federated bundles, and bundle-related flags), but several of those options are not wired into runtime behavior.

## Feature gaps vs upstream README

| Upstream README feature | Rust main branch status | Gap / notes |
| --- | --- | --- |
| **JWT SVID fetching and persistence** | Not implemented | Config parsing supports `jwt_svids`, `jwt_bundle_file_name`, and related modes, but no runtime code fetches JWT SVIDs or writes JWT tokens. |
| **JWT bundle writing** | Not implemented | Same as above—configuration exists, but no Workload API interaction to fetch/write JWT bundles. |
| **Federated bundle inclusion** (`include_federated_domains`) | Not implemented | Configuration flag exists, but there is no logic to enumerate/write federated bundles. |
| **Intermediate certs in bundle** (`add_intermediates_to_bundle`) | Not implemented | Config flag exists but no code path merges intermediates into the bundle. |
| **SVID selection hints / filtering** (`hint`, `omit_expired`) | Not implemented | Configuration options exist but are not used in the Workload API fetching flow. |
| **JWT file mode handling** (`jwt_bundle_file_mode`, `jwt_svid_file_mode`) | Not implemented | File mode parsing exists but no JWT files are written, so modes are unused. |
| **Bundle-specific file mode** | Partial | Bundle writes reuse the cert file mode rather than a dedicated bundle mode option. |

## Suggested next steps

1. Implement JWT SVID and JWT bundle retrieval and persistence paths (including file-mode application).
2. Add support for `include_federated_domains` and `add_intermediates_to_bundle` when writing bundles.
3. Wire `hint`/`omit_expired` into X.509 SVID selection logic.
4. Consider aligning bundle file mode handling with upstream behavior if it differs.

