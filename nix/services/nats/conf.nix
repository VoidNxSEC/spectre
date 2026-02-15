# NATS Server configuration generator
# Pure function: config attrset -> nats.conf string
#
# Note: store_dir is intentionally omitted from the config file.
# It is passed via CLI --store_dir to allow runtime override via
# NATS_STORE_DIR environment variable without regenerating the derivation.
{ lib }:

cfg:

let
  inherit (lib) optionalString concatStringsSep;
  inherit (builtins) toString;

  # Convert boolean to NATS config format
  boolToStr = b: if b then "true" else "false";

  # Indent a block of text
  indent = text: concatStringsSep "\n" (
    map (line: if line == "" then "" else "  ${line}")
      (lib.splitString "\n" text)
  );

  # Core server block
  serverBlock = ''
    server_name: ${cfg.serverName}
    listen: 0.0.0.0:${toString cfg.clientPort}
    http: 0.0.0.0:${toString cfg.monitorPort}
    max_connections: ${toString cfg.limits.maxConnections}
  '';

  # JetStream block
  jetstreamBlock = optionalString cfg.jetstream.enabled ''

    jetstream {
      max_mem: ${cfg.jetstream.maxMemory}
      max_file: ${cfg.jetstream.maxFile}
    }
  '';

  # Logging block
  loggingBlock = ''

    debug: ${boolToStr cfg.logging.debug}
    trace: false
    logtime: true
  '';

  # Cluster block
  clusterBlock = optionalString (cfg.cluster != null) ''

    cluster {
      name: ${cfg.cluster.name}
      listen: 0.0.0.0:${toString cfg.clusterPort}
    ${optionalString (cfg.cluster ? routes && cfg.cluster.routes != []) ''
      routes = [
    ${concatStringsSep "\n" (map (r: "    ${r}") cfg.cluster.routes)}
      ]
    ''}
    }
  '';

  # Authorization block
  authBlock = optionalString (cfg.authorization != null) ''

    authorization {
    ${optionalString (cfg.authorization ? users) (
      concatStringsSep "\n" (map (user: ''
      user: ${user.name}
      password: ${user.password}
    '') cfg.authorization.users)
    )}
    ${optionalString (cfg.authorization ? token) ''
      token: ${cfg.authorization.token}
    ''}
    }
  '';

in
  serverBlock + jetstreamBlock + loggingBlock + clusterBlock + authBlock
