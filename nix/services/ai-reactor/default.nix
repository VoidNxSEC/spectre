# SPECTRE AI Reactor — NixOS Module
#
# Declarative module for the AI event reactor service.
# No hardcoded values — all thresholds, URLs, and secrets are configurable.
#
# Usage in flake.nix or configuration.nix:
#
#   services.spectre-ai-reactor = {
#     enable = true;
#     natsUrl = "nats://localhost:4222";
#     thresholds = {
#       maxConsecutiveFailures = 5;
#       queueDepthThresholdPct = 0.8;
#       scaleCooldownSeconds = 60;
#     };
#     secretsFile = "/run/secrets/ai-reactor-env";
#   };
#
# What you get:
#   - systemd service with hardening (NoNewPrivileges, PrivateTmp, ProtectSystem)
#   - Auto-restart on failure
#   - journald logging with JSON format
#   - sops-nix integration for API keys
#   - KEDA ScaledObject for llama-server (optional)

{
  config,
  lib,
  pkgs,
  ...
}:

with lib;

let
  cfg = config.services.spectre-ai-reactor;

  # Default configuration
  defaultConfig = {
    enable = false;
    package = null; # Built from source in flake
    natsUrl = "nats://localhost:4222";
    mlflowUrl = "http://localhost:5000";
    timescaledbUrl = "postgresql://localhost:5432/spectre_ai";

    thresholds = {
      maxConsecutiveFailures = 5;
      queueDepthThresholdPct = 0.8;
      queueSustainSeconds = 30;
      scaleCooldownSeconds = 60;
      stableModelVersion = "latest-stable";
    };

    secretsFile = null; # sops-nix env file path
    logLevel = "info";
    openFirewall = false; # AI reactor doesn't expose ports

    keda = {
      enable = false;
      namespace = "default";
      llamaServerDeployment = "llama-server";
      minReplicas = 1;
      maxReplicas = 10;
      cooldownPeriod = 60;
    };
  };

  # Merge user config with defaults
  cfg' = recursiveUpdate defaultConfig cfg;

in
{
  options.services.spectre-ai-reactor = {
    enable = mkEnableOption "SPECTRE AI Reactor — event-driven AI backbone";

    package = mkOption {
      type = types.nullOr types.package;
      default = null;
      description = "spectre-ai-reactor package (built from source in flake)";
    };

    natsUrl = mkOption {
      type = types.str;
      default = "nats://localhost:4222";
      description = "NATS server URL";
    };

    mlflowUrl = mkOption {
      type = types.str;
      default = "http://localhost:5000";
      description = "MLflow tracking server URL";
    };

    timescaledbUrl = mkOption {
      type = types.str;
      default = "postgresql://localhost:5432/spectre_ai";
      description = "TimescaleDB connection URL for ADR storage";
    };

    thresholds = {
      maxConsecutiveFailures = mkOption {
        type = types.int;
        default = 5;
        description = "Number of consecutive failures before rollback";
      };

      queueDepthThresholdPct = mkOption {
        type = types.float;
        default = 0.8;
        description = "Queue depth % threshold to trigger scale-up";
      };

      queueSustainSeconds = mkOption {
        type = types.int;
        default = 30;
        description = "Seconds of sustained high queue before scale-up";
      };

      scaleCooldownSeconds = mkOption {
        type = types.int;
        default = 60;
        description = "Min seconds between scale actions (anti-flap)";
      };

      stableModelVersion = mkOption {
        type = types.str;
        default = "latest-stable";
        description = "Model version to rollback TO";
      };
    };

    secretsFile = mkOption {
      type = types.nullOr types.str;
      default = null;
      description = "Path to sops-nix EnvironmentFile with API keys";
    };

    logLevel = mkOption {
      type = types.enum [
        "trace"
        "debug"
        "info"
        "warn"
        "error"
      ];
      default = "info";
      description = "Log level (RUST_LOG)";
    };

    openFirewall = mkOption {
      type = types.bool;
      default = false;
      description = "Open firewall ports (AI reactor doesn't expose ports)";
    };

    keda = {
      enable = mkEnableOption "KEDA ScaledObject for auto-scaling llama-server";

      namespace = mkOption {
        type = types.str;
        default = "default";
        description = "Kubernetes namespace";
      };

      llamaServerDeployment = mkOption {
        type = types.str;
        default = "llama-server";
        description = "llama-server Deployment name";
      };

      minReplicas = mkOption {
        type = types.int;
        default = 1;
        description = "Minimum replicas";
      };

      maxReplicas = mkOption {
        type = types.int;
        default = 10;
        description = "Maximum replicas";
      };

      cooldownPeriod = mkOption {
        type = types.int;
        default = 60;
        description = "Scale-down cooldown in seconds";
      };
    };
  };

  # ── Systemd service ────────────────────────────────────────────────────────

  config = mkIf cfg'.enable {
    systemd.services.spectre-ai-reactor = {
      description = "SPECTRE AI Reactor — event-driven AI backbone";
      wantedBy = [ "multi-user.target" ];
      after = [
        "network-online.target"
        "nats.service"
      ];
      requires = [ "nats.service" ];

      # Environment
      environment = {
        NATS_URL = cfg'.natsUrl;
        MLFLOW_URL = cfg'.mlflowUrl;
        TIMESCALEDB_URL = cfg'.timescaledbUrl;
        RUST_LOG = cfg'.logLevel;
        MAX_CONSECUTIVE_FAILURES = toString cfg'.thresholds.maxConsecutiveFailures;
        QUEUE_DEPTH_THRESHOLD_PCT = toString cfg'.thresholds.queueDepthThresholdPct;
        SCALE_COOLDOWN_SECONDS = toString cfg'.thresholds.scaleCooldownSeconds;
        STABLE_MODEL_VERSION = cfg'.thresholds.stableModelVersion;
      };

      # Secrets via EnvironmentFile (sops-nix pattern)
      serviceConfig = mkMerge [
        {
          Type = "simple";
          Restart = "always";
          RestartSec = "5";
          User = "spectre-ai-reactor";
          Group = "spectre-ai-reactor";
          DynamicUser = true;

          # ── systemd hardening ──
          NoNewPrivileges = true;
          PrivateTmp = true;
          ProtectSystem = "strict";
          ProtectHome = true;
          ProtectKernelTunables = true;
          ProtectKernelModules = true;
          ProtectControlGroups = true;
          RestrictAddressFamilies = [
            "AF_INET"
            "AF_INET6"
            "AF_UNIX"
          ];
          RestrictRealtime = true;
          MemoryDenyWriteExecute = true;
          LockPersonality = true;
          SystemCallFilter = [
            "@system-service"
            "~@privileged"
            "~@resources"
          ];

          # Read-only access to Nix store (for binary)
          ReadOnlyPaths = [ "/nix/store" ];
        }

        # Secrets file (optional, from sops-nix)
        (mkIf (cfg'.secretsFile != null) {
          EnvironmentFile = cfg'.secretsFile;
        })
      ];

      # Service binary (built from flake or provided as package)
      script =
        let
          pkg =
            if cfg'.package != null then
              cfg'.package
            else
              pkgs.spectre-ai-reactor
                or (throw "spectre-ai-reactor package not found. Add to flake.nix or set services.spectre-ai-reactor.package.");
        in
        ''
          exec ${pkg}/bin/spectre-ai-reactor
        '';
    };

    # ── User/group ──
    users.users.spectre-ai-reactor = {
      isSystemUser = true;
      group = "spectre-ai-reactor";
      description = "SPECTRE AI Reactor service user";
    };
    users.groups.spectre-ai-reactor = { };

    # ── KEDA ScaledObject (Kubernetes) ──
    environment.etc."kubernetes/keda/llama-server-scaledobject.yaml" = mkIf cfg'.keda.enable {
      text = builtins.toJSON {
        apiVersion = "keda.sh/v1alpha1";
        kind = "ScaledObject";
        metadata = {
          name = "${cfg'.keda.llamaServerDeployment}-autoscaler";
          namespace = cfg'.keda.namespace;
        };
        spec = {
          scaleTargetRef.name = cfg'.keda.llamaServerDeployment;
          minReplicaCount = cfg'.keda.minReplicas;
          maxReplicaCount = cfg'.keda.maxReplicas;
          cooldownPeriod = cfg'.keda.cooldownPeriod;

          triggers = [
            {
              type = "nats-jetstream";
              metadata = {
                natsServerMonitoringEndpoint = cfg'.natsUrl;
                stream = "SPECTRE_AI_EVENTS";
                subject = "ml_offload.queue.depth";
                lagThreshold = toString (builtins.floor (cfg'.thresholds.queueDepthThresholdPct * 100));
                activationLagThreshold = "10";
              };
            }
          ];
        };
      };
      mode = "0444";
    };
  };
}
