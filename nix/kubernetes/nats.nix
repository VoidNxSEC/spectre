# Kubernetes NATS Deployment for in-cluster use
# Provides NATS server so spectre-proxy /ready probe works in K8s
{ lib, k8sLib, config }:

with k8sLib;

let
  labels = mkLabels {
    name = "nats";
    instance = config.instance;
  };
in
[
  # --- Deployment ---
  {
    apiVersion = "apps/v1";
    kind = "Deployment";

    metadata = mkMetadata {
      name = "nats";
      namespace = config.namespace;
      inherit labels;
    };

    spec = {
      replicas = 1;
      selector.matchLabels = labels;

      template = {
        metadata = {
          inherit labels;
        };

        spec = {
          containers = [
            {
              name = "nats";
              image = "nats:2.12-alpine";
              args = [
                "--js"
                "--name" "spectre-nats"
                "--http_port" "8222"
                "--max_connections" "256"
              ];

              ports = [
                { name = "client"; containerPort = 4222; protocol = "TCP"; }
                { name = "monitor"; containerPort = 8222; protocol = "TCP"; }
                { name = "cluster"; containerPort = 6222; protocol = "TCP"; }
              ];

              livenessProbe = {
                httpGet = { path = "/healthz"; port = 8222; };
                initialDelaySeconds = 5;
                periodSeconds = 10;
              };

              readinessProbe = {
                httpGet = { path = "/healthz"; port = 8222; };
                initialDelaySeconds = 2;
                periodSeconds = 5;
              };

              resources = mkResources {
                requestsCpu = "50m";
                requestsMemory = "64Mi";
                limitsCpu = "200m";
                limitsMemory = "256Mi";
              };

              securityContext = mkSecurityContext {
                runAsNonRoot = true;
                runAsUser = 1000;
                allowPrivilegeEscalation = false;
                readOnlyRootFilesystem = false; # NATS needs /tmp for JetStream
              };

              volumeMounts = [
                { name = "jetstream-data"; mountPath = "/data/jetstream"; }
              ];
            }
          ];

          volumes = [
            { name = "jetstream-data"; emptyDir = {}; }
          ];
        };
      };
    };
  }

  # --- Service ---
  {
    apiVersion = "v1";
    kind = "Service";

    metadata = mkMetadata {
      name = "nats";
      namespace = config.namespace;
      inherit labels;
    };

    spec = {
      type = "ClusterIP";
      selector = labels;

      ports = [
        { name = "client"; port = 4222; targetPort = "client"; protocol = "TCP"; }
        { name = "monitor"; port = 8222; targetPort = "monitor"; protocol = "TCP"; }
      ];
    };
  }
]
