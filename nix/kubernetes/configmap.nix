# Kubernetes ConfigMap for spectre-proxy
{ lib, k8sLib, config }:

with k8sLib;

let
  cfg = config;
  labels = mkLabels {
    name = "spectre-proxy";
    instance = cfg.instance;
  };
in
{
  apiVersion = "v1";
  kind = "ConfigMap";

  metadata = mkMetadata {
    name = "spectre-proxy";
    namespace = cfg.namespace;
    inherit labels;
  };

  data = {
    # NATS Configuration
    NATS_URL = cfg.nats.url;

    # Upstream Service
    NEUTRON_URL = cfg.neutron.url;

    # Rate Limiting
    RATE_LIMIT_RPS = toString cfg.rateLimit.rps;
    RATE_LIMIT_BURST = toString cfg.rateLimit.burst;

    # Observability
    OTEL_TRACES_SAMPLER_ARG = cfg.observability.samplingRate;

    # Logging
    RUST_LOG = cfg.logging.level;
    SPECTRE_LOG_FORMAT = cfg.logging.format;
    SPECTRE_ENV = cfg.environment;

    # TLS (disabled with Ingress)
    TLS_ENABLED = "false";
  } // lib.optionalAttrs (cfg.observability.otlpEndpoint != null) {
    OTEL_EXPORTER_OTLP_ENDPOINT = cfg.observability.otlpEndpoint;
  };
}
