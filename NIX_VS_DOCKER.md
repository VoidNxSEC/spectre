# Nix vs Docker: Why We Chose Nix-First

**Decision Date**: 2026-02-15
**Status**: Active
**Related ADR**: ADR-0037 (Nix-First Kubernetes Orchestration)

---

## TL;DR

**SPECTRE uses Nix to build container images, not Docker.**

- ✅ Nix builds OCI images via `dockerTools.buildLayeredImage`
- ✅ Kubernetes runs containers (containerd/CRI-O)
- ✅ docker-compose runs infrastructure services (NATS, Jaeger, etc.)
- ❌ No Dockerfile for application builds
- ❌ No `docker build` in CI/CD

---

## The Problem with Traditional Docker

### 1. Non-Reproducible Builds
```dockerfile
FROM rust:bookworm AS builder
RUN cargo build --release  # ← Different timestamps = different image
```

**Issue**: Two builds from same source can produce different binaries:
- Layer timestamps vary
- Network fetches can get different versions
- Base image updates break reproducibility

### 2. Build Tool Duplication
```yaml
# Before: THREE build systems
- Cargo builds Rust binaries
- Docker builds containers
- Helm/Kustomize builds K8s manifests

# After: ONE build system
- Nix builds everything
```

### 3. Docker Daemon Requirement
```bash
# Traditional CI
- Install Docker daemon (privileged)
- docker build (needs daemon)
- docker push (needs daemon)

# Nix CI
- nix build .#spectre-proxy-image  # No daemon
- skopeo copy (no daemon)
```

**CI/CD Impact**: Docker-in-Docker is complex, slow, and insecure.

---

## The Nix Solution

### 1. Hash-Based Reproducibility
```nix
# nix/images/spectre-proxy.nix
pkgs.dockerTools.buildLayeredImage {
  name = "spectre-proxy";
  tag = "nix-${builtins.substring 0 8 (self.rev or "dev")}";
  contents = [ cacert bashInteractive coreutils ];
  config.Cmd = [ "${spectre-proxy}/bin/spectre-proxy" ];
}
```

**Guarantee**: Same source hash → Same binary → Same image (bit-for-bit).

### 2. Unified Build System
```bash
# One flake.nix rules them all:
nix build .#spectre-proxy              # Rust binary
nix build .#spectre-proxy-image        # OCI image
nix build .#kubernetes-manifests-dev   # K8s YAML
nix develop                            # Dev shell
```

### 3. No Docker Daemon Needed
```bash
# Build image
nix build .#spectre-proxy-image
# → /nix/store/abc123-docker-image-spectre-proxy.tar.gz

# Load to daemon (optional, for testing)
docker load < result

# Push to registry (no daemon)
skopeo copy docker-archive:result docker://registry/spectre:latest
```

---

## Comparison Table

| Feature | Docker | Nix |
|---------|--------|-----|
| **Reproducibility** | ❌ Non-deterministic | ✅ Hash-guaranteed |
| **Build Caching** | Layer-based (fragile) | Content-addressable (robust) |
| **Daemon Required** | ✅ Yes (privileged) | ❌ No |
| **Layer Deduplication** | Manual (`COPY --from`) | Automatic (Nix store) |
| **Cross-compilation** | ❌ QEMU emulation | ✅ Native (Nix cross) |
| **Offline Builds** | ❌ Needs network | ✅ Fully hermetic |
| **Binary Size** | ~30MB (Distroless) | ~30MB (Nix minimal) |
| **CI/CD Complexity** | High (Docker-in-Docker) | Low (single binary) |
| **Team Learning Curve** | Low (widespread) | Medium (Nix syntax) |

---

## What We Kept

### docker-compose.yml (Infrastructure Only)
```yaml
# Used ONLY for local development infrastructure
services:
  nats:          # Message bus
  timescaledb:   # Metrics storage
  neo4j:         # Graph database
  jaeger:        # Tracing
  prometheus:    # Metrics collection
  grafana:       # Dashboards

# NO application services built here
```

**Why keep it?**
- Mature ecosystem for service orchestration
- Easy `docker-compose up -d` for dev environment
- Not used in production (Kubernetes is)

### Docker as Container Runtime
```bash
# Kubernetes uses containerd/CRI-O (not Docker)
# But locally you can still use Docker:
docker load < $(nix build .#spectre-proxy-image --print-out-paths)
docker run -p 3000:3000 spectre-proxy:nix-dev
```

**Docker's role**: Container runtime, NOT build tool.

---

## Migration Path (What Changed)

### Before (Dockerfile)
```dockerfile
# Dockerfile
FROM rust:bookworm AS builder
WORKDIR /build
COPY Cargo.* ./
RUN cargo build --release
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /build/target/release/spectre-proxy /
ENTRYPOINT ["/spectre-proxy"]
```

### After (Nix)
```nix
# nix/images/spectre-proxy.nix
{ lib, dockerTools, cacert, spectre-proxy, ... }:
dockerTools.buildLayeredImage {
  name = "spectre-proxy";
  tag = "nix-${version}";
  contents = [ cacert bashInteractive coreutils ];
  config = {
    Cmd = [ "${spectre-proxy}/bin/spectre-proxy" ];
    User = "1000:1000";
    ExposedPorts = { "3000/tcp" = {}; };
  };
  maxLayers = 100;  # Automatic deduplication
}
```

### CI/CD Changes
```yaml
# Before: Job 8 - Docker Build (REMOVED)
docker:
  - docker build -t spectre-proxy .
  - docker push

# After: Job 9 - Nix Image
nix-image:
  - nix build .#spectre-proxy-image
  - skopeo copy (if pushing)
```

---

## Trade-offs Accepted

### ✅ Gains
1. **Reproducibility**: Build once, deploy anywhere (guaranteed)
2. **Simplicity**: One build system for everything
3. **Security**: No privileged Docker daemon in CI
4. **Speed**: Nix cache >> Docker layer cache
5. **Offline**: Hermetic builds work without network

### ❌ Costs
1. **Learning Curve**: Team must learn Nix language
2. **Community Size**: Fewer Nix+K8s examples than Dockerfile+K8s
3. **Tooling**: Some tools expect Dockerfile (rare, workarounds exist)

**Decision**: Reproducibility and simplicity outweigh learning curve.

---

## Common Questions

### "Why not just use Distroless with Docker?"
Distroless solves image size, not reproducibility. You still get:
- Non-deterministic builds (timestamps)
- Docker daemon requirement
- Build tool duplication

### "What about Docker Buildx multi-platform?"
Nix cross-compilation is more robust:
```bash
# Nix cross-compile to ARM64
nix build .#packages.aarch64-linux.spectre-proxy-image

# Docker uses QEMU emulation (slow, can break)
docker buildx build --platform linux/arm64
```

### "How do I debug containers without shell?"
```bash
# Nix includes bashInteractive in contents
docker run -it spectre-proxy:nix-dev bash

# Or use kubectl debug (ephemeral containers)
kubectl debug pod/spectre-proxy-xxx -it --image=busybox
```

### "What if I need to use Dockerfile later?"
Nix can generate Dockerfiles from derivations:
```nix
# Generate Dockerfile from Nix build
dockerTools.buildImageWithNixDb # Includes /nix/store
```

But you probably won't need to.

---

## Implementation Guide

### Building Images
```bash
# Build
nix build .#spectre-proxy-image

# Inspect
tar -tzf result | head -20

# Load to Docker (optional)
docker load < result

# Push to registry (no Docker daemon)
skopeo copy \
  docker-archive:result \
  docker://ghcr.io/yourorg/spectre-proxy:latest
```

### CI/CD Integration
```yaml
# .github/workflows/ci.yml
nix-image:
  runs-on: ubuntu-latest
  steps:
    - uses: cachix/install-nix-action@v22
    - run: nix build .#spectre-proxy-image
    - run: skopeo copy docker-archive:$(readlink result) docker://registry/image
```

### Kubernetes Deployment
```bash
# Manifests reference Nix-built images
nix build .#kubernetes-manifests-dev
kubectl apply -f result

# Or use deploy app
nix run .#deploy-dev
```

---

## Future Considerations

### When to Re-evaluate
- **If** Nix becomes unmaintained (unlikely, CNCF interest growing)
- **If** Docker BuildKit adds content-addressable guarantees (monitoring)
- **If** Team size grows significantly and Nix training becomes bottleneck

### Evolution Path
1. **Current (2026-02)**: Nix-only container builds
2. **Future (2026-06+)**: Nix for entire NixOS-based K8s cluster?
3. **Advanced**: NixOS containers (systemd, declarative services)

---

## References

- **ADR-0037**: Nix-First Kubernetes Orchestration
- **Nix Pills**: https://nixos.org/guides/nix-pills/
- **dockerTools docs**: https://nixos.org/manual/nixpkgs/stable/#sec-pkgs-dockerTools
- **Reproducible Builds**: https://reproducible-builds.org/

---

## Conclusion

**SPECTRE's philosophy: One build system, maximum reproducibility.**

Docker remains useful as a **container runtime** (locally and via Kubernetes), but Nix is the **build tool** of record. This decision eliminates complexity, ensures reproducibility, and aligns with SPECTRE's commitment to production-grade infrastructure.

*"Build once, run anywhere" — but actually.*
