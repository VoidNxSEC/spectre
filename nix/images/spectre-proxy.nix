# Nix-based container image for spectre-proxy
# Builds a reproducible OCI image without Docker daemon
{ pkgs, lib, spectre-proxy }:

pkgs.dockerTools.buildLayeredImage {
  name = "spectre-proxy";
  tag = "nix-${lib.substring 0 8 spectre-proxy.src.rev or "dev"}";

  # Use minimal base with just what we need
  contents = [
    pkgs.cacert  # CA certificates for HTTPS
    pkgs.bashInteractive  # For debugging
    pkgs.coreutils  # Basic utils
  ];

  # Configuration
  config = {
    # Entrypoint
    Cmd = [ "${spectre-proxy}/bin/spectre-proxy" ];

    # Expose ports
    ExposedPorts = {
      "3000/tcp" = {};
    };

    # Environment
    Env = [
      "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
      "RUST_BACKTRACE=1"
    ];

    # User (non-root)
    User = "1000:1000";

    # Working directory
    WorkingDir = "/";

    # Labels
    Labels = {
      "org.opencontainers.image.source" = "https://github.com/your-org/spectre";
      "org.opencontainers.image.description" = "SPECTRE Proxy - Zero-trust API Gateway";
      "org.opencontainers.image.licenses" = "MIT";
      "io.nix.built-by" = "nix";
    };
  };

  # Create layers for better caching
  maxLayers = 100;
}
