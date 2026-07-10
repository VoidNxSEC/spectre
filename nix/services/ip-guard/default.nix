# IP Guard — NixOS Module
#
# Declarative module for IP Guard license compliance verification.
# One systemd service per blockchain network with full hardening.
{
  config,
  lib,
  pkgs,
  ...
}:

with lib;

let
  cfg = config.services.ip-guard;

  chainOpts =
    { name, ... }:
    {
      options = {
        enable = mkEnableOption "IP Guard for ${name}";
        rpcUrl = mkOption {
          type = types.str;
          description = "RPC URL";
        };
        contractAddress = mkOption {
          type = types.str;
          default = "";
        };
        chainId = mkOption { type = types.int; };
        pollInterval = mkOption {
          type = types.str;
          default = "30s";
        };
      };
    };
in
{
  options.services.ip-guard = {
    enable = mkEnableOption "IP Guard — blockchain license compliance";
    package = mkOption {
      type = types.nullOr types.package;
      default = null;
    };
    dataDir = mkOption {
      type = types.str;
      default = "/var/lib/ip-guard";
    };
    secretsFile = mkOption {
      type = types.nullOr types.str;
      default = null;
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
    };

    chains = {
      ethereum = mkOption {
        type = types.submodule chainOpts;
        default = {
          enable = false;
          rpcUrl = "https://eth.llamarpc.com";
          chainId = 1;
        };
      };
      polygon = mkOption {
        type = types.submodule chainOpts;
        default = {
          enable = false;
          rpcUrl = "https://polygon-rpc.com";
          chainId = 137;
        };
      };
      anvil = mkOption {
        type = types.submodule chainOpts;
        default = {
          enable = true;
          rpcUrl = "http://localhost:8545";
          chainId = 31337;
        };
      };
    };
  };

  config = mkIf cfg.enable {
    systemd.tmpfiles.rules = [
      "d ${cfg.dataDir} 0700 ip-guard ip-guard -"
      "d ${cfg.dataDir}/cache 0700 ip-guard ip-guard -"
    ];

    systemd.services = mkMerge (
      mapAttrsToList (
        chainName: chainCfg:
        mkIf chainCfg.enable {
          "ip-guard-${chainName}" = {
            description = "IP Guard — ${chainName}";
            wantedBy = [ "multi-user.target" ];
            after = [ "network-online.target" ];

            environment = {
              RUST_LOG = cfg.logLevel;
              IPGUARD_RPC_URL = chainCfg.rpcUrl;
              IPGUARD_CONTRACT = chainCfg.contractAddress;
              NEUTRON_LICENSE_CACHE = "${cfg.dataDir}/cache";
            };

            serviceConfig = mkMerge [
              {
                Type = "simple";
                Restart = "always";
                RestartSec = "10";
                User = "ip-guard";
                Group = "ip-guard";
                DynamicUser = true;
                NoNewPrivileges = true;
                PrivateTmp = true;
                ProtectSystem = "strict";
                ProtectHome = true;
                MemoryDenyWriteExecute = true;
                LockPersonality = true;
                ReadWritePaths = [ cfg.dataDir ];
                ReadOnlyPaths = [ "/nix/store" ];
                SystemCallFilter = [
                  "@system-service"
                  "~@privileged"
                  "~@resources"
                ];
              }
              (mkIf (cfg.secretsFile != null) { EnvironmentFile = cfg.secretsFile; })
            ];

            script =
              let
                pkg =
                  if cfg.package != null then cfg.package else pkgs.ip-guard or (throw "ip-guard package not found");
              in
              "exec ${pkg}/bin/ip-guard status";
          };
        }
      ) cfg.chains
    );

    users.users.ip-guard = {
      isSystemUser = true;
      group = "ip-guard";
    };
    users.groups.ip-guard = { };
  };
}
