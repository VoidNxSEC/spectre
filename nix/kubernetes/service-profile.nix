# Linkerd ServiceProfile CRD for spectre-proxy
# Defines per-route traffic policies: timeouts, retry budgets, and route classification.
#
# Routes:
#   POST /ingest  — 10s timeout, retryable (idempotent in practice via NATS dedup)
#   GET  /health  — no timeout override, not retryable (fast probe, no side effects)
#
# Apply: kubectl apply -f result
# Verify: linkerd viz routes deployment/spectre-proxy
{ lib }:

[
  {
    apiVersion = "linkerd.io/v1alpha2";
    kind = "ServiceProfile";

    metadata = {
      name = "spectre-proxy.default.svc.cluster.local";
      namespace = "default";
    };

    spec = {
      # Global retry budget — caps total retries across all retryable routes
      retryBudget = {
        retryRatio = 0.2;          # Allow up to 20% extra requests as retries
        minRetriesPerSecond = 10;  # Minimum retry floor regardless of RPS
        ttl = "10s";               # Window for retry ratio accounting
      };

      routes = [
        {
          name = "POST /ingest";
          condition = {
            method = "POST";
            pathRegex = "/ingest";
          };
          timeout = "10s";       # Upstream NATS + neutron round-trip budget
          isRetryable = true;    # NATS dedup key prevents duplicate processing
        }
        {
          name = "GET /health";
          condition = {
            method = "GET";
            pathRegex = "/health";
          };
          # No timeout override — health probes use Linkerd default (10s)
          isRetryable = false;   # Probe failures should surface immediately
        }
      ];
    };
  }
]
