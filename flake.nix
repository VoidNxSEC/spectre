{
  description = "SPECTRE Fleet - Enterprise-Grade AI Agent Framework";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    sops-nix = {
      url = "github:Mic92/sops-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      sops-nix,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Rust toolchain
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
          ];
        };

        # Common build inputs
        commonBuildInputs = with pkgs; [
          # Rust toolchain
          rustToolchain
          cargo-watch
          cargo-edit
          cargo-audit

          # Build tools
          pkg-config
          gcc
          cmake

          # System libraries
          openssl
          sqlite

          # NATS CLI for debugging
          natscli

          # Docker for dev environment
          docker-compose

          # Database CLIs
          postgresql
          neo4j

          # Python for Python services
          python3
          uv

          # Development tools
          git
          jq
          ripgrep
          fd

          # Security tools
          sops
          age
          ssh-to-age
        ];

      in
      {
        # Development shell
        devShells.default = pkgs.mkShell {
          buildInputs = commonBuildInputs;

          shellHook = ''
            echo "🚀 SPECTRE Fleet Development Environment"
            echo ""
            echo "Available services:"
            echo "  • NATS:         docker-compose up nats"
            echo "  • TimescaleDB:  docker-compose up timescaledb"
            echo "  • Neo4j:        docker-compose up neo4j"
            echo "  • All:          docker-compose up -d"
            echo ""
            echo "Rust toolchain: $(rustc --version)"
            echo "Cargo:          $(cargo --version)"
            echo "NATS CLI:       $(nats --version 2>/dev/null || echo 'not available')"
            echo ""
            echo "Quick start:"
            echo "  1. docker-compose up -d"
            echo "  2. cargo build"
            echo "  3. cargo test"
            echo ""

            # Set environment variables for development
            export RUST_BACKTRACE=1
            export RUST_LOG=debug
            export NATS_URL=nats://localhost:4222
            export TIMESCALEDB_URL=postgresql://spectre:spectre_dev_password@localhost:5432/spectre_observability
            export NEO4J_URI=neo4j://localhost:7687
            export NEO4J_USER=neo4j
            export NEO4J_PASSWORD=spectre_dev_password
          '';
        };

        # Packages (to be implemented)
        packages = {
          # Core infrastructure
          # spectre-proxy = ...;
          # spectre-observability = ...;

          # Default package
          default = pkgs.hello; # Placeholder
        };

        # Checks (CI/CD)
        checks = {
          # Rust formatting
          fmt =
            pkgs.runCommand "check-rust-fmt"
              {
                buildInputs = [ rustToolchain ];
              }
              ''
                cd ${self}
                cargo fmt -- --check
                touch $out
              '';

          # Rust clippy
          clippy =
            pkgs.runCommand "check-rust-clippy"
              {
                buildInputs = [ rustToolchain ] ++ commonBuildInputs;
              }
              ''
                cd ${self}
                cargo clippy --all-targets --all-features -- -D warnings
                touch $out
              '';

          # Tests (requires NATS running)
          # test = pkgs.runCommand "check-tests" {
          #   buildInputs = [ rustToolchain ] ++ commonBuildInputs;
          # } ''
          #   cd ${self}
          #   cargo test --all-features
          #   touch $out
          # '';
        };

        # NixOS module (to be implemented)
        # nixosModules.spectre = import ./nix/modules/spectre.nix;
      }
    );
}
