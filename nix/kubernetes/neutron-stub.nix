# Kubernetes manifests for neutron stub (go-httpbin)
# Deploys a lightweight HTTP echo server as a stand-in for the neutron service.
# Purpose: validate mTLS between spectre-proxy and neutron inside the service mesh
# without building the full NEXUS/neutron image (CUDA, Ray, pgvector).
#
# Service URL: http://neutron.default.svc.cluster.local:8000
# Linkerd mTLS: enabled via annotation on pod template
{ lib, k8sLib }:

with k8sLib;

let
  labels = mkLabels {
    name = "neutron";
    instance = "stub";
    version = "2.14.0";
  };
in
[
  # --- Deployment ---
  {
    apiVersion = "apps/v1";
    kind = "Deployment";

    metadata = mkMetadata {
      name = "neutron";
      namespace = "default";
      inherit labels;
      annotations = {
        "app.kubernetes.io/description" = "Stub neutron for service mesh mTLS validation";
      };
    };

    spec = {
      replicas = 1;
      selector.matchLabels = labels;

      template = {
        metadata = {
          inherit labels;
          annotations = {
            # Participate in the Linkerd mesh so mTLS is established
            "linkerd.io/inject" = "enabled";
          };
        };

        spec = {
          containers = [
            (mkContainer {
              name = "neutron";
              image = "ghcr.io/mccutchen/go-httpbin:v2.14.0";

              ports = [
                { name = "http"; containerPort = 8080; protocol = "TCP"; }
              ];

              livenessProbe = mkHttpProbe {
                path = "/get";
                port = "http";
                initialDelaySeconds = 5;
                periodSeconds = 10;
              };

              readinessProbe = mkHttpProbe {
                path = "/get";
                port = "http";
                initialDelaySeconds = 2;
                periodSeconds = 5;
              };

              resources = mkResources {
                requestsCpu = "50m";
                requestsMemory = "64Mi";
                limitsCpu = "200m";
                limitsMemory = "128Mi";
              };
            })
          ];
        };
      };
    };
  }

  # --- Service (matches neutron.default.svc.cluster.local:8000) ---
  {
    apiVersion = "v1";
    kind = "Service";

    metadata = mkMetadata {
      name = "neutron";
      namespace = "default";
      inherit labels;
    };

    spec = {
      type = "ClusterIP";
      selector = labels;

      ports = [
        { name = "http"; port = 8000; targetPort = 8080; protocol = "TCP"; }
      ];
    };
  }
]
