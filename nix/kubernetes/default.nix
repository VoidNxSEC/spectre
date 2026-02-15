# Main Kubernetes module - composes all resources
{ lib, pkgs }:

let
  k8sLib = import ../lib/k8s.nix { inherit lib; };

  # Default configuration
  mkConfig = {
    instance ? "default",
    namespace ? null,
    version ? "0.1.0",

    # Image
    image,

    # Deployment
    replicas ? 2,
    antiAffinity ? true,

    # Resources
    resources ? {
      requests = { cpu = "100m"; memory = "128Mi"; };
      limits = { cpu = "500m"; memory = "512Mi"; };
    },

    # NATS
    nats ? {
      url = "nats://nats.default.svc.cluster.local:4222";
    },

    # Upstream
    neutron ? {
      url = "http://neutron.default.svc.cluster.local:8000";
    },

    # Rate limiting
    rateLimit ? {
      rps = 100;
      burst = 200;
    },

    # Observability
    observability ? {
      otlpEndpoint = null;
      samplingRate = "0.1";
    },

    # Logging
    logging ? {
      level = "info";
      format = "json";
    },

    # Environment
    environment ? "production",

    # Ingress
    ingress ? {
      enabled = true;
      className = "nginx";
      host = "spectre.example.com";
      tls = {
        enabled = true;
        issuer = "letsencrypt-prod";
      };
    },
  }: {
    inherit instance namespace version image replicas antiAffinity resources;
    inherit nats neutron rateLimit observability logging environment ingress;
  };

  # Generate all Kubernetes manifests
  mkManifests = config:
    let
      cfg = mkConfig config;
    in
    [
      (import ./deployment.nix { inherit lib; k8sLib = k8sLib; config = cfg; })
      (import ./service.nix { inherit lib; k8sLib = k8sLib; config = cfg; })
      (import ./configmap.nix { inherit lib; k8sLib = k8sLib; config = cfg; })
    ] ++ lib.optional cfg.ingress.enabled
      (import ./ingress.nix { inherit lib; k8sLib = k8sLib; config = cfg; });

  # Convert manifests to YAML
  manifestsToYAML = manifests:
    k8sLib.mergeManifests manifests;

in
{
  inherit mkConfig mkManifests manifestsToYAML;

  # Pre-configured environments
  environments = {
    dev = mkConfig {
      instance = "dev";
      replicas = 1;
      antiAffinity = false;
      resources = {
        requests = { cpu = "50m"; memory = "64Mi"; };
        limits = { cpu = "200m"; memory = "256Mi"; };
      };
      nats.url = "nats://localhost:4222";
      neutron.url = "http://localhost:8000";
      rateLimit = { rps = 1000; burst = 2000; };
      observability = {
        otlpEndpoint = "http://localhost:4317";
        samplingRate = "1.0";
      };
      logging = { level = "debug"; format = "pretty"; };
      environment = "development";
      ingress = {
        enabled = true;
        className = "nginx";
        host = "spectre-dev.local";
        tls.enabled = false;
      };
      # Image will be set by flake
      image = "spectre-proxy:dev";
    };

    prod = mkConfig {
      instance = "prod";
      replicas = 3;
      antiAffinity = true;
      resources = {
        requests = { cpu = "200m"; memory = "256Mi"; };
        limits = { cpu = "1000m"; memory = "1Gi"; };
      };
      nats.url = "nats://nats-cluster.nats.svc.cluster.local:4222";
      neutron.url = "http://neutron.production.svc.cluster.local:8000";
      rateLimit = { rps = 100; burst = 200; };
      observability = {
        otlpEndpoint = "http://tempo-distributor.observability.svc.cluster.local:4317";
        samplingRate = "0.05";
      };
      logging = { level = "info"; format = "json"; };
      environment = "production";
      ingress = {
        enabled = true;
        className = "nginx";
        host = "spectre.production.com";
        tls = {
          enabled = true;
          issuer = "letsencrypt-prod";
        };
      };
      # Image will be set by flake
      image = "spectre-proxy:latest";
    };
  };
}
