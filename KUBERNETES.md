# SPECTRE Kubernetes Deployment Guide

**Complete guide for deploying SPECTRE Proxy on Kubernetes with Ingress, cert-manager, and Prometheus.**

---

## 📋 Prerequisites

### Required
- **Kubernetes** 1.25+ (tested on 1.28)
- **Helm** 3.12+
- **kubectl** configured with cluster access

### Recommended Infrastructure
- **Ingress Controller**: nginx-ingress
- **TLS**: cert-manager with Let's Encrypt
- **Metrics**: Prometheus Operator
- **Tracing**: Tempo/Jaeger with OTLP

---

## 🚀 Quick Start

### 1. Install Prerequisites

```bash
# Install nginx-ingress
helm upgrade --install ingress-nginx ingress-nginx \
  --repo https://kubernetes.github.io/ingress-nginx \
  --namespace ingress-nginx --create-namespace

# Install cert-manager
kubectl apply -f https://github.com/cert-manager/cert-manager/releases/download/v1.13.0/cert-manager.yaml

# Create Let's Encrypt ClusterIssuer
kubectl apply -f - <<EOF
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: letsencrypt-prod
spec:
  acme:
    server: https://acme-v02.api.letsencrypt.org/directory
    email: your-email@example.com
    privateKeySecretRef:
      name: letsencrypt-prod
    solvers:
    - http01:
        ingress:
          class: nginx
EOF
```

### 2. Deploy SPECTRE Proxy

**Development:**
```bash
helm install spectre-dev ./charts/spectre-proxy \
  -f ./charts/spectre-proxy/values-dev.yaml \
  --set image.tag=dev
```

**Production:**
```bash
helm install spectre-prod ./charts/spectre-proxy \
  -f ./charts/spectre-proxy/values-prod.yaml \
  --set image.tag=v0.1.0 \
  --set secrets.jwtSecret=$JWT_SECRET \
  --set ingress.host=spectre.yourdomain.com
```

### 3. Verify Deployment

```bash
# Check pod status
kubectl get pods -l app.kubernetes.io/name=spectre-proxy

# Check logs
kubectl logs -l app.kubernetes.io/name=spectre-proxy --tail=100 -f

# Test health endpoint
kubectl port-forward svc/spectre-prod-spectre-proxy 8080:80
curl http://localhost:8080/health
# Expected: OK

# Test readiness
curl http://localhost:8080/ready
# Expected: {"status":"ready","nats":true,"upstream":...}
```

---

## ⚙️ Configuration

### Essential Values

| Parameter | Description | Default | Required |
|-----------|-------------|---------|----------|
| `image.repository` | Container image | `ghcr.io/your-org/spectre-proxy` | Yes |
| `image.tag` | Image tag | `""` (uses appVersion) | Yes |
| `secrets.jwtSecret` | JWT signing key | `""` | **YES** |
| `ingress.host` | Domain name | `spectre.example.com` | Yes |
| `nats.url` | NATS server URL | `nats://nats.default:4222` | Yes |
| `neutron.url` | Upstream service URL | `http://neutron.default:8000` | Yes |

### Security Values

| Parameter | Description | Default |
|-----------|-------------|---------|
| `ingress.tls.enabled` | Enable TLS | `true` |
| `ingress.tls.issuer` | cert-manager issuer | `letsencrypt-prod` |
| `podSecurityContext.runAsNonRoot` | Run as non-root | `true` |
| `securityContext.readOnlyRootFilesystem` | Read-only root FS | `true` |

### Scaling Values

| Parameter | Description | Default |
|-----------|-------------|---------|
| `replicaCount` | Initial replicas (if HPA disabled) | `2` |
| `autoscaling.enabled` | Enable HPA | `true` |
| `autoscaling.minReplicas` | Minimum replicas | `2` |
| `autoscaling.maxReplicas` | Maximum replicas | `10` |
| `autoscaling.targetCPUUtilizationPercentage` | CPU target | `70` |
| `podDisruptionBudget.minAvailable` | Min pods during disruption | `1` |

### Observability Values

| Parameter | Description | Default |
|-----------|-------------|---------|
| `observability.otlpEndpoint` | OTLP traces endpoint | `""` (disabled) |
| `observability.samplingRate` | Trace sampling (0.0-1.0) | `"0.1"` (10%) |
| `metrics.enabled` | Enable Prometheus metrics | `true` |
| `metrics.serviceMonitor.enabled` | Create ServiceMonitor | `true` |
| `logging.level` | Log level | `info` |
| `logging.format` | Log format | `json` |

---

## 📊 Monitoring & Observability

### Prometheus Metrics

The proxy exposes metrics at `/metrics`:

**Key Metrics:**
- `spectre_proxy_requests_total` - Total requests (labels: method, path, status)
- `spectre_proxy_request_duration_seconds` - Request latency histogram
- `spectre_events_published_total` - Total events published to NATS

**ServiceMonitor:**
```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: spectre-proxy
spec:
  endpoints:
  - port: metrics
    path: /metrics
    interval: 30s
```

Automatically created when `metrics.serviceMonitor.enabled: true`.

### Distributed Tracing

Configure OTLP endpoint for traces:

```yaml
observability:
  otlpEndpoint: "http://tempo-distributor.observability:4317"
  samplingRate: "0.1"  # 10% sampling
```

Traces include:
- HTTP request spans
- NATS publish spans
- Upstream proxy spans
- Correlation ID propagation

### Logs

Structured JSON logs to stdout:

```bash
# View logs
kubectl logs -l app.kubernetes.io/name=spectre-proxy -f

# Query with Loki (if installed)
{app="spectre-proxy"} | json | level="error"
```

---

## 🔒 Security Best Practices

### 1. Secrets Management

**DO NOT** hardcode secrets in `values.yaml`!

**Option A: Helm install with --set**
```bash
helm install spectre ./charts/spectre-proxy \
  --set secrets.jwtSecret=$JWT_SECRET
```

**Option B: External Secrets Operator** (recommended)
```yaml
apiVersion: external-secrets.io/v1beta1
kind: ExternalSecret
metadata:
  name: spectre-secrets
spec:
  secretStoreRef:
    name: vault-backend
  target:
    name: spectre-prod-spectre-proxy
  data:
  - secretKey: JWT_SECRET
    remoteRef:
      key: spectre/jwt-secret
```

**Option C: Sealed Secrets**
```bash
echo -n "your-secret" | kubectl create secret generic spectre-secrets \
  --from-file=JWT_SECRET=/dev/stdin \
  --dry-run=client -o yaml | \
  kubeseal -o yaml > sealed-secret.yaml
```

### 2. Network Policies

Restrict pod-to-pod communication:

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: spectre-proxy
spec:
  podSelector:
    matchLabels:
      app.kubernetes.io/name: spectre-proxy
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: ingress-nginx
    ports:
    - protocol: TCP
      port: 3000
  egress:
  - to:
    - podSelector:
        matchLabels:
          app: nats
    ports:
    - protocol: TCP
      port: 4222
  - to:
    - podSelector:
        matchLabels:
          app: neutron
    ports:
    - protocol: TCP
      port: 8000
  # Allow DNS
  - to:
    - namespaceSelector: {}
    ports:
    - protocol: UDP
      port: 53
```

### 3. Pod Security Standards

Chart enforces **restricted** security context:
- `runAsNonRoot: true`
- `readOnlyRootFilesystem: true`
- `allowPrivilegeEscalation: false`
- Capabilities: drop ALL

---

## 🔧 Troubleshooting

### Pod not ready

```bash
# Check pod status
kubectl describe pod <pod-name>

# Common issues:
# 1. Readiness probe failing
kubectl logs <pod-name>
# Check: NATS connection, upstream connectivity

# 2. ImagePullBackOff
kubectl get events
# Fix: Check image.repository and imagePullSecrets

# 3. CrashLoopBackOff
kubectl logs <pod-name> --previous
# Fix: Check JWT_SECRET is set, env vars correct
```

### Ingress not working

```bash
# Check ingress status
kubectl get ingress
kubectl describe ingress spectre-proxy

# Common issues:
# 1. DNS not pointing to ingress IP
nslookup spectre.example.com
# Should point to nginx-ingress LoadBalancer IP

# 2. TLS certificate not ready
kubectl get certificate
kubectl describe certificate spectre-proxy-tls
# Wait for cert-manager to issue certificate

# 3. 502 Bad Gateway
kubectl port-forward svc/spectre-proxy 8080:80
curl http://localhost:8080/health
# If works locally, issue is ingress config
```

### High CPU/Memory

```bash
# Check resource usage
kubectl top pods -l app.kubernetes.io/name=spectre-proxy

# Scale manually
kubectl scale deployment spectre-proxy --replicas=5

# Check HPA status
kubectl get hpa
kubectl describe hpa spectre-proxy

# Increase resource limits
helm upgrade spectre ./charts/spectre-proxy \
  --reuse-values \
  --set resources.limits.cpu=2000m \
  --set resources.limits.memory=2Gi
```

### NATS connection failed

```bash
# Test NATS connectivity from pod
kubectl exec -it <pod-name> -- sh
wget -O- http://nats:8222/varz

# Check NATS_URL config
kubectl get configmap spectre-proxy -o yaml | grep NATS_URL

# Port-forward NATS for debugging
kubectl port-forward svc/nats 4222:4222
```

---

## 📈 Scaling Guidelines

### Horizontal Scaling (HPA)

**Development:**
- Min: 1 replica
- Max: 3 replicas
- Disable HPA for predictable behavior

**Production:**
- Min: 3 replicas (HA)
- Max: 20 replicas
- Target: 70% CPU, 80% Memory
- Stabilization: 300s (prevent flapping)

**Calculation:**
```
Replicas = ceil(current * (current_metric / target_metric))
```

### Vertical Scaling (Resources)

**Baseline (development):**
- Requests: 50m CPU, 64Mi memory
- Limits: 200m CPU, 256Mi memory

**Production:**
- Requests: 200m CPU, 256Mi memory
- Limits: 1000m CPU, 1Gi memory

**High Traffic:**
- Requests: 500m CPU, 512Mi memory
- Limits: 2000m CPU, 2Gi memory

### Rate Limiting

**Per-pod capacity:**
- Default: 100 RPS, burst 200
- Recommended: Set based on upstream capacity / replicas

**Example:**
- Upstream: 1000 RPS max
- Replicas: 5
- Per-pod: 200 RPS, burst 400

```yaml
rateLimit:
  rps: 200
  burst: 400
```

---

## 🔄 Upgrade Strategy

### Rolling Update

Default strategy: `RollingUpdate`
- `maxUnavailable: 0` - no downtime
- `maxSurge: 1` - one extra pod during rollout

```bash
# Upgrade with new image
helm upgrade spectre ./charts/spectre-proxy \
  --reuse-values \
  --set image.tag=v0.2.0

# Monitor rollout
kubectl rollout status deployment/spectre-proxy

# Rollback if needed
kubectl rollout undo deployment/spectre-proxy
```

### Blue-Green Deployment

```bash
# Deploy new version as separate release
helm install spectre-green ./charts/spectre-proxy \
  -f values-prod.yaml \
  --set image.tag=v0.2.0 \
  --set fullnameOverride=spectre-green

# Test green deployment
kubectl port-forward svc/spectre-green 8080:80

# Switch ingress to green
kubectl patch ingress spectre \
  -p '{"spec":{"rules":[{"host":"...","http":{"paths":[{"backend":{"service":{"name":"spectre-green"}}}]}}}]}}'

# Delete blue after validation
helm uninstall spectre-blue
```

---

## 📦 Resource Requirements

### Minimum Cluster Requirements

**Development:**
- Nodes: 1
- CPU: 1 core
- Memory: 2Gi
- Storage: 10Gi

**Production:**
- Nodes: 3+ (multi-AZ)
- CPU: 4 cores
- Memory: 8Gi
- Storage: 50Gi

### Per-Pod Resource Usage

**Idle:**
- CPU: ~10m
- Memory: ~50Mi

**Under load (100 RPS):**
- CPU: ~100m
- Memory: ~128Mi

**Under load (1000 RPS):**
- CPU: ~500m
- Memory: ~256Mi

---

## 🌍 Multi-Environment Setup

```
environments/
├── dev/
│   ├── values.yaml (inherits from charts/spectre-proxy/values-dev.yaml)
│   └── secrets.yaml (sealed)
├── staging/
│   ├── values.yaml
│   └── secrets.yaml
└── prod/
    ├── values.yaml (inherits from values-prod.yaml)
    └── secrets.yaml
```

**Deploy to each environment:**
```bash
# Dev
helm upgrade --install spectre-dev ./charts/spectre-proxy \
  -f charts/spectre-proxy/values-dev.yaml \
  -f environments/dev/values.yaml \
  --namespace dev

# Staging
helm upgrade --install spectre-staging ./charts/spectre-proxy \
  -f charts/spectre-proxy/values.yaml \
  -f environments/staging/values.yaml \
  --namespace staging

# Production
helm upgrade --install spectre-prod ./charts/spectre-proxy \
  -f charts/spectre-proxy/values-prod.yaml \
  -f environments/prod/values.yaml \
  --namespace production
```

---

## 📚 Additional Resources

- [Helm Chart Reference](./charts/spectre-proxy/README.md)
- [API Documentation](./API.md)
- [Architecture Overview](./README.md)
- [Security Practices](./SECURITY.md)

---

## 🐛 Support

- **Issues**: https://github.com/your-org/spectre/issues
- **Discussions**: https://github.com/your-org/spectre/discussions
- **Slack**: #spectre-support
