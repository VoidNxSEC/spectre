# Kubernetes Service for spectre-proxy
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
  kind = "Service";

  metadata = mkMetadata {
    name = "spectre-proxy";
    namespace = cfg.namespace;
    inherit labels;
  };

  spec = {
    type = "ClusterIP";

    selector = labels;

    ports = [
      {
        name = "http";
        port = 80;
        targetPort = "http";
        protocol = "TCP";
      }
      {
        name = "metrics";
        port = 9090;
        targetPort = "http";
        protocol = "TCP";
      }
    ];
  };
}
