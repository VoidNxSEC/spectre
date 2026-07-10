# Reusable NATS Server module for SPECTRE and monorepo projects
#
# API follows the same pattern as nix/kubernetes/default.nix:
#   mkConfig        : attrset -> config       - Merge defaults with overrides
#   mkNatsConf      : config  -> string       - Generate nats.conf text
#   mkServerPackage : config  -> derivation   - Wrapper script (executable)
#   environments    : { dev, prod }           - Pre-configured presets
#
# Usage from another project:
#   let
#     natsModule = import ../spectre/nix/services/nats { inherit (pkgs) lib; inherit pkgs; };
#     myNats = natsModule.mkServerPackage (natsModule.mkConfig {
#       serverName = "my-project-nats";
#     });
#   in { ... }
{ lib, pkgs }:

let
  mkNatsConf = import ./conf.nix { inherit lib; };

  mkConfig = {
    serverName    ? "spectre-nats-dev",
    clientPort    ? 4222,
    monitorPort   ? 8222,
    clusterPort   ? 6222,

    jetstream ? {
      enabled  = true;
      storeDir = "/tmp/spectre-nats-dev";
      maxMemory = "128MB";
      maxFile   = "512MB";
    },

    logging ? {
      debug = true;
    },

    limits ? {
      maxConnections = 256;
    },

    cluster       ? null,
    authorization ? null,
  }: {
    inherit serverName clientPort monitorPort clusterPort;
    inherit jetstream logging limits cluster authorization;
  };

  mkServerPackage = config:
    let
      confText = mkNatsConf config;
      confFile = pkgs.writeText "nats.conf" confText;
      defaultStoreDir = config.jetstream.storeDir;
    in
    pkgs.writeShellScriptBin "nats-server-spectre" ''
      set -euo pipefail

      NATS_STORE_DIR="''${NATS_STORE_DIR:-${defaultStoreDir}}"
      mkdir -p "$NATS_STORE_DIR"

      echo "SPECTRE NATS Server (${config.serverName})"
      echo ""
      echo "  Client:    nats://0.0.0.0:${toString config.clientPort}"
      echo "  Monitor:   http://0.0.0.0:${toString config.monitorPort}"
      echo "  Cluster:   0.0.0.0:${toString config.clusterPort}"
      echo "  JetStream: ${if config.jetstream.enabled then "enabled" else "disabled"}"
      echo "  Store dir: $NATS_STORE_DIR"
      echo ""

      exec ${pkgs.nats-server}/bin/nats-server \
        --config ${confFile} \
        --store_dir "$NATS_STORE_DIR"
    '';

in
{
  inherit mkConfig mkNatsConf mkServerPackage;

  # Pre-configured environments
  environments = {
    dev = mkConfig {
      serverName    = "spectre-nats-dev";
      clientPort    = 4222;
      monitorPort   = 8222;
      clusterPort   = 6222;
      jetstream = {
        enabled   = true;
        storeDir  = "/tmp/spectre-nats-dev";
        maxMemory = "128MB";
        maxFile   = "512MB";
      };
      logging.debug = true;
      limits.maxConnections = 256;
    };

    prod = mkConfig {
      serverName    = "spectre-nats-prod";
      clientPort    = 4222;
      monitorPort   = 8222;
      clusterPort   = 6222;
      jetstream = {
        enabled   = true;
        storeDir  = "/var/lib/nats/jetstream";
        maxMemory = "1GB";
        maxFile   = "10GB";
      };
      logging.debug = false;
      limits.maxConnections = 4096;
    };
  };
}
