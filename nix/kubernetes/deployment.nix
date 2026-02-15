# Kubernetes Deployment for spectre-proxy
{ lib, k8sLib, config }:

with k8sLib;

let
  cfg = config;
  labels = mkLabels {
    name = "spectre-proxy";
    instance = cfg.instance;
    version = cfg.version;
  };
in
{
  apiVersion = "apps/v1";
  kind = "Deployment";

  metadata = mkMetadata {
    name = "spectre-proxy";
    namespace = cfg.namespace;
    inherit labels;
  };

  spec = {
    replicas = cfg.replicas;

    selector.matchLabels = labels;

    strategy = {
      type = "RollingUpdate";
      rollingUpdate = {
        maxUnavailable = 0;
        maxSurge = 1;
      };
    };

    template = {
      metadata = {
        inherit labels;
        annotations = {
          "prometheus.io/scrape" = "true";
          "prometheus.io/port" = "3000";
          "prometheus.io/path" = "/metrics";
        };
      };

      spec = {
        serviceAccountName = "spectre-proxy";

        securityContext = {
          runAsNonRoot = true;
          runAsUser = 1000;
          fsGroup = 1000;
          seccompProfile.type = "RuntimeDefault";
        };

        containers = [
          (mkContainer {
            name = "spectre-proxy";
            image = cfg.image;

            ports = [
              { name = "http"; containerPort = 3000; protocol = "TCP"; }
            ];

            envFrom = [
              { configMapRef.name = "spectre-proxy"; }
              { secretRef.name = "spectre-proxy"; }
            ];

            livenessProbe = mkHttpProbe {
              path = "/health";
              port = "http";
              initialDelaySeconds = 10;
              periodSeconds = 10;
            };

            readinessProbe = mkHttpProbe {
              path = "/ready";
              port = "http";
              initialDelaySeconds = 5;
              periodSeconds = 5;
            };

            startupProbe = mkHttpProbe {
              path = "/health";
              port = "http";
              initialDelaySeconds = 0;
              periodSeconds = 2;
              failureThreshold = 30;
            };

            resources = mkResources {
              requestsCpu = cfg.resources.requests.cpu;
              requestsMemory = cfg.resources.requests.memory;
              limitsCpu = cfg.resources.limits.cpu;
              limitsMemory = cfg.resources.limits.memory;
            };

            securityContext = mkSecurityContext {
              runAsNonRoot = true;
              runAsUser = 1000;
              allowPrivilegeEscalation = false;
              readOnlyRootFilesystem = true;
            };
          })
        ];

        # Anti-affinity for HA
        affinity = lib.optionalAttrs cfg.antiAffinity {
          podAntiAffinity.preferredDuringSchedulingIgnoredDuringExecution = [{
            weight = 100;
            podAffinityTerm = {
              labelSelector.matchLabels = labels;
              topologyKey = "kubernetes.io/hostname";
            };
          }];
        };
      };
    };
  };
}
