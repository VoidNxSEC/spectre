# Kubernetes helper functions for generating manifests
{ lib }:

with lib;

rec {
  # Convert Nix attrset to Kubernetes YAML
  toYAML = data: builtins.toJSON data;

  # Generate standard labels
  mkLabels = { name, instance, version ? "0.1.0" }: {
    "app.kubernetes.io/name" = name;
    "app.kubernetes.io/instance" = instance;
    "app.kubernetes.io/version" = version;
    "app.kubernetes.io/managed-by" = "nix";
  };

  # Generate metadata with labels
  mkMetadata = { name, namespace ? null, labels ? {}, annotations ? {} }:
    {
      inherit name;
      inherit labels annotations;
    } // optionalAttrs (namespace != null) { inherit namespace; };

  # Generate container spec
  mkContainer = {
    name,
    image,
    ports ? [],
    env ? [],
    envFrom ? [],
    resources ? {},
    livenessProbe ? null,
    readinessProbe ? null,
    startupProbe ? null,
    securityContext ? {},
  }: {
    inherit name image;
  } // optionalAttrs (ports != []) { inherit ports; }
    // optionalAttrs (env != []) { inherit env; }
    // optionalAttrs (envFrom != []) { inherit envFrom; }
    // optionalAttrs (resources != {}) { inherit resources; }
    // optionalAttrs (livenessProbe != null) { inherit livenessProbe; }
    // optionalAttrs (readinessProbe != null) { inherit readinessProbe; }
    // optionalAttrs (startupProbe != null) { inherit startupProbe; }
    // optionalAttrs (securityContext != {}) { inherit securityContext; };

  # Generate HTTP GET probe
  mkHttpProbe = {
    path,
    port,
    initialDelaySeconds ? 10,
    periodSeconds ? 10,
    timeoutSeconds ? 3,
    failureThreshold ? 3,
  }: {
    httpGet = {
      inherit path port;
    };
    inherit initialDelaySeconds periodSeconds timeoutSeconds failureThreshold;
  };

  # Generate resource requirements
  mkResources = {
    requestsCpu ? "100m",
    requestsMemory ? "128Mi",
    limitsCpu ? "500m",
    limitsMemory ? "512Mi",
  }: {
    requests = {
      cpu = requestsCpu;
      memory = requestsMemory;
    };
    limits = {
      cpu = limitsCpu;
      memory = limitsMemory;
    };
  };

  # Generate security context (non-root, read-only)
  mkSecurityContext = {
    runAsNonRoot ? true,
    runAsUser ? 1000,
    allowPrivilegeEscalation ? false,
    readOnlyRootFilesystem ? true,
  }: {
    inherit runAsNonRoot runAsUser allowPrivilegeEscalation readOnlyRootFilesystem;
    capabilities.drop = [ "ALL" ];
  };

  # Merge manifests into single YAML file
  mergeManifests = manifests:
    let
      yamlDocs = map toYAML manifests;
    in
    concatStringsSep "\n---\n" yamlDocs;
}
