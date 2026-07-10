# Kubernetes Ingress for spectre-proxy
{ lib, k8sLib, config }:

with k8sLib;

let
  cfg = config;
  labels = mkLabels {
    name = "spectre-proxy";
    instance = cfg.instance;
  };
in
lib.optionalAttrs cfg.ingress.enabled {
  apiVersion = "networking.k8s.io/v1";
  kind = "Ingress";

  metadata = mkMetadata {
    name = "spectre-proxy";
    namespace = cfg.namespace;
    inherit labels;
    annotations = {
      "nginx.ingress.kubernetes.io/ssl-redirect" =
        if cfg.ingress.tls.enabled then "true" else "false";
    } // lib.optionalAttrs cfg.ingress.tls.enabled {
      "cert-manager.io/cluster-issuer" = cfg.ingress.tls.issuer;
    };
  };

  spec = {
    ingressClassName = cfg.ingress.className;

    rules = [{
      host = cfg.ingress.host;
      http.paths = [{
        path = "/";
        pathType = "Prefix";
        backend.service = {
          name = "spectre-proxy";
          port.name = "http";
        };
      }];
    }];
  } // lib.optionalAttrs cfg.ingress.tls.enabled {
    tls = [{
      hosts = [ cfg.ingress.host ];
      secretName = "spectre-proxy-tls";
    }];
  };
}
