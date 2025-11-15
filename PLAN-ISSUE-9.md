# Implementation Plan: SPIRE Server Manifests & Deployment (Issue #9)

## Overview
Create Kubernetes manifests and Make targets to deploy the SPIRE server in the kind cluster, using certificates from `./artifacts/certs` via Kubernetes Secrets.

## Current State Analysis

### Existing Infrastructure
- ✅ Kind cluster setup (`make cluster-up` / `make cluster-down`)
- ✅ Artifacts directory structure (`./artifacts/`)
- ✅ Kubeconfig management
- ❌ No SPIRE server manifests
- ❌ No certificate Secret creation
- ❌ No deployment automation

### Assumptions
- Certificates are generated in `./artifacts/certs/` (prerequisite from other tasks)
- SPIRE server image: `ghcr.io/spiffe/spire-server:latest` (or similar)
- SPIRE server requires TLS certificates for:
  - Server TLS (listening on port 8081)
  - CA bundle for signing
  - Bootstrap bundle

## Implementation Plan

### Phase 1: Directory Structure & Manifests

#### 1.1 Create Deployment Directory Structure
```
deploy/
└── spire/
    └── server/
        ├── namespace.yaml
        ├── configmap.yaml
        ├── statefulset.yaml (or deployment.yaml)
        ├── service.yaml
        └── kustomization.yaml (optional, for organization)
```

#### 1.2 Namespace Manifest (`deploy/spire/server/namespace.yaml`)
- Create `spire-server` namespace
- Add labels for identification

#### 1.3 ConfigMap Manifest (`deploy/spire/server/configmap.yaml`)
- Contains `server.conf` with:
  - Server configuration (data store, log level, etc.)
  - Plugin configurations
  - Trust domain
  - Port configuration (8081 for API, 8082 for health)
  - References to mounted certificate paths

#### 1.4 StatefulSet Manifest (`deploy/spire/server/statefulset.yaml`)
**Why StatefulSet?**
- SPIRE server typically uses persistent storage for data store
- Ordered pod management
- Stable network identity

**Key Components:**
- Container: SPIRE server image
- Volume mounts:
  - ConfigMap for `server.conf`
  - Secrets for TLS certificates:
    - Server certificate/key
    - CA bundle
    - Bootstrap bundle
- Environment variables (if needed)
- Resource limits/requests
- Readiness/liveness probes
- Service account (if needed for K8s node attestation)

#### 1.5 Service Manifest (`deploy/spire/server/service.yaml`)
- ClusterIP service for internal communication
- Ports:
  - 8081: SPIRE API (TLS)
  - 8082: Health check (HTTP)
- Selector matching StatefulSet labels

### Phase 2: Secret Management

#### 2.1 Certificate Secret Creation Strategy
**Option A: Separate Secrets per certificate type**
- `spire-server-tls` - server cert/key
- `spire-server-ca` - CA bundle
- `spire-server-bootstrap` - bootstrap bundle

**Option B: Single Secret with multiple keys**
- `spire-server-certs` with keys:
  - `server.crt`, `server.key`
  - `ca.crt`, `ca.key`
  - `bootstrap.crt`

**Recommendation:** Option A for better separation of concerns

#### 2.2 Secret Manifest Files (Optional)
- Create template manifests in `deploy/spire/server/secrets/`
- Or use `kubectl create secret` commands in Makefile

**Certificate File Mapping:**
Assuming certs in `./artifacts/certs/`:
- `server.crt` / `server.key` → TLS secret
- `ca.crt` / `ca.key` → CA secret
- `bootstrap.crt` → Bootstrap secret

### Phase 3: Makefile Integration

#### 3.1 Prerequisites Check
```makefile
deploy-spire-server: check-cluster check-certs
```

**Check Functions:**
- `check-cluster`: Verify cluster exists and kubeconfig is valid
- `check-certs`: Verify required certificate files exist in `./artifacts/certs/`

#### 3.2 Deployment Target
```makefile
.PHONY: deploy-spire-server
deploy-spire-server: check-prerequisites
	@# Apply namespace
	@# Create/update Secrets from certs
	@# Apply ConfigMap
	@# Apply StatefulSet
	@# Apply Service
	@# Wait for pod readiness
```

**Implementation Details:**
- Use `kubectl apply` for idempotency
- Use `kubectl create secret` with `--dry-run=client -o yaml | kubectl apply -f -` for Secrets
- Wait for StatefulSet rollout: `kubectl rollout status statefulset/spire-server -n spire-server`
- Wait for pod ready: `kubectl wait --for=condition=ready pod -l app=spire-server -n spire-server --timeout=300s`

#### 3.3 Cleanup Target
```makefile
.PHONY: undeploy-spire-server
undeploy-spire-server:
	@# Delete resources in reverse order
	@# StatefulSet → Service → ConfigMap → Secrets → Namespace
```

**Integration with cluster-down:**
- Option: Add cleanup to `cluster-down` target
- Option: Keep separate for granular control

### Phase 4: Configuration Details

#### 4.1 SPIRE Server Configuration (`server.conf`)
**Key Sections:**
```hcl
server {
    bind_address = "0.0.0.0"
    bind_port = "8081"
    trust_domain = "example.org"
    data_dir = "/run/spire/data"
    log_level = "INFO"
    
    ca_subject {
        country = ["US"]
        organization = ["SPIRE"]
    }
}

plugins {
    DataStore "sql" {
        database_type = "sqlite3"
        connection_string = "/run/spire/data/datastore.sqlite3"
    }
    
    NodeAttestor "k8s_psat" {
        service_account_allow_list = ["spire-server:spire-server"]
    }
}
```

**Certificate Paths:**
- TLS cert: `/run/spire/secrets/server.crt`
- TLS key: `/run/spire/secrets/server.key`
- CA cert: `/run/spire/secrets/ca.crt`

#### 4.2 Volume Mounts in StatefulSet
```yaml
volumeMounts:
  - name: config
    mountPath: /run/spire/config
    readOnly: true
  - name: server-tls
    mountPath: /run/spire/secrets
    readOnly: true
  - name: data
    mountPath: /run/spire/data
volumes:
  - name: config
    configMap:
      name: spire-server-config
  - name: server-tls
    secret:
      secretName: spire-server-tls
  - name: data
    emptyDir: {}  # Or persistentVolumeClaim for production
```

### Phase 5: Testing & Validation

#### 5.1 Manual Testing Steps
1. Generate certificates (prerequisite)
2. Run `make deploy-spire-server`
3. Verify pod status: `kubectl get pods -n spire-server`
4. Check logs: `kubectl logs -n spire-server spire-server-0`
5. Verify service: `kubectl get svc -n spire-server`
6. Test readiness: `kubectl exec -n spire-server spire-server-0 -- spire-server healthcheck`

#### 5.2 Idempotency Testing
- Run `make deploy-spire-server` multiple times
- Verify no errors on re-application
- Verify pod remains healthy

#### 5.3 Prerequisite Failure Testing
- Run without cluster → should fail fast
- Run without certs → should fail fast with clear error

## File Structure (Final)

```
deploy/
└── spire/
    └── server/
        ├── namespace.yaml
        ├── configmap.yaml
        ├── statefulset.yaml
        ├── service.yaml
        └── README.md (optional, for documentation)

Makefile (updated with new targets)
```

## Implementation Checklist

- [ ] Create `deploy/spire/server/` directory structure
- [ ] Create namespace manifest
- [ ] Create ConfigMap manifest with `server.conf`
- [ ] Create StatefulSet manifest with proper volume mounts
- [ ] Create Service manifest
- [ ] Add `check-cluster` function to Makefile
- [ ] Add `check-certs` function to Makefile
- [ ] Add `deploy-spire-server` target to Makefile
- [ ] Add `undeploy-spire-server` target to Makefile
- [ ] Test deployment with existing cluster
- [ ] Test idempotency
- [ ] Test prerequisite checks
- [ ] Test cleanup
- [ ] Update README.md with deployment instructions (if needed)

## Open Questions / Decisions Needed

1. **StatefulSet vs Deployment:**
   - Recommendation: StatefulSet for persistent data store
   - Decision: Confirm with team

2. **Storage:**
   - Use `emptyDir` for development?
   - Or `persistentVolumeClaim` for persistence?
   - Recommendation: Start with `emptyDir`, document PVC option

3. **SPIRE Server Image:**
   - Which image/tag to use?
   - Recommendation: `ghcr.io/spiffe/spire-server:latest` or specific version

4. **Trust Domain:**
   - What trust domain to use?
   - Recommendation: `spiffe-helper.local` or configurable

5. **Certificate File Names:**
   - Exact names of cert files in `./artifacts/certs/`?
   - Need to coordinate with certificate generation task

6. **Kubernetes Node Attestation:**
   - Enable `k8s_psat` plugin?
   - Or use simpler attestation for initial deployment?

## Dependencies

- **Prerequisites:**
  - Kind cluster running (`make cluster-up`)
  - Certificates generated in `./artifacts/certs/`
  - `kubectl` available and configured

- **Future Integration:**
  - SPIRE Agent deployment (separate task)
  - Certificate generation automation (separate task)

## Notes

- All manifests should follow Kubernetes best practices
- Use labels consistently for resource selection
- Consider adding resource limits for development environment
- Health checks are critical for readiness detection
- Secrets should never be committed to git (already in .gitignore)

