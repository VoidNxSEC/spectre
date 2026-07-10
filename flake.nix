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

          # NATS
          natscli
          nats-server

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

          # Benchmarking
          hey

          # Chaos engineering (Phase 3 — network fault injection)
          toxiproxy
        ];

      in
      {
        # Development shells
        devShells = {
          # Default Rust development shell
          default = pkgs.mkShell {
            buildInputs = commonBuildInputs;

            shellHook = ''
              echo "🚀 SPECTRE Fleet Development Environment"
              echo ""
              echo "Available services:"
              echo "  • NATS:         nix run .#nats"
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
              echo "Chaos engineering (Task #47):"
              echo "  cargo build --release && ./scripts/chaos-test.sh"
              echo "  ./scripts/chaos-test.sh --phase 2  (single phase)"
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

          # Kubernetes operations shell
          kubernetes = pkgs.mkShell {
            buildInputs = with pkgs; [
              # Kubernetes tools
              kubectl
              kubernetes-helm
              k9s
              kustomize

              # Service mesh
              linkerd

              # Container tools
              skopeo
              dive

              # Observability
              prometheus
              grafana

              # Development tools
              jq
              yq-go
              git
            ];

            shellHook = ''
              echo "☸️  SPECTRE Kubernetes Environment"
              echo ""
              echo "Available tools:"
              echo "  • kubectl:  $(kubectl version --client --short 2>/dev/null || echo 'Kubernetes CLI')"
              echo "  • helm:     $(helm version --short 2>/dev/null || echo 'Helm package manager')"
              echo "  • k9s:      Kubernetes TUI"
              echo ""
              echo "Nix commands:"
              echo "  • nix build .#kubernetes-manifests-dev     - Generate dev manifests"
              echo "  • nix build .#kubernetes-manifests-prod    - Generate prod manifests"
              echo "  • nix build .#spectre-proxy-image          - Build container image"
              echo "  • nix run .#deploy-dev                     - Deploy to dev cluster"
              echo "  • nix run .#deploy-prod                    - Deploy to prod cluster"
              echo ""
            '';
          };
        };

        # Packages
        packages =
          let
            # Import Kubernetes module
            k8sModule = import ./nix/kubernetes {
              inherit (pkgs) lib;
              inherit pkgs;
            };

            # Import NATS module
            natsModule = import ./nix/services/nats {
              inherit (pkgs) lib;
              inherit pkgs;
            };

            # Build spectre-proxy Rust binary
            spectre-proxy = pkgs.rustPlatform.buildRustPackage {
              pname = "spectre-proxy";
              version = "0.1.0";
              src = ./.;
              cargoLock.lockFile = ./Cargo.lock;
              nativeBuildInputs = with pkgs; [ pkg-config ];
              buildInputs = with pkgs; [ openssl ];
              cargoBuildFlags = [ "-p" "spectre-proxy" ];
              cargoTestFlags = [ "-p" "spectre-proxy" ];
              doCheck = false; # Integration tests require NATS
            };

            # Container image (no Docker daemon needed)
            spectre-proxy-image = pkgs.dockerTools.buildLayeredImage {
              name = "spectre-proxy";
              tag = "nix-${builtins.substring 0 8 (self.rev or "dev")}";
              contents = with pkgs; [ cacert bashInteractive coreutils ];
              config = {
                Cmd = [ "${spectre-proxy}/bin/spectre-proxy" ];
                User = "1000:1000";
                ExposedPorts = { "3000/tcp" = {}; };
                Env = [
                  "RUST_LOG=info"
                  "SPECTRE_ENV=production"
                ];
              };
              maxLayers = 100;
            };

            # Generate Kubernetes manifests
            mkManifests = env:
              let
                manifests = k8sModule.mkManifests k8sModule.environments.${env};
                yaml = k8sModule.manifestsToYAML manifests;
              in
              pkgs.writeTextFile {
                name = "spectre-k8s-manifests-${env}";
                text = yaml;
              };
          in
          {
            # Core packages
            inherit spectre-proxy spectre-proxy-image;

            # Kubernetes manifests
            kubernetes-manifests-dev = mkManifests "dev";
            kubernetes-manifests-prod = mkManifests "prod";

            # NATS server packages
            nats-server-dev = natsModule.mkServerPackage natsModule.environments.dev;
            nats-server-prod = natsModule.mkServerPackage natsModule.environments.prod;

            # Service mesh manifests
            neutron-stub-manifests = pkgs.writeTextFile {
              name = "neutron-stub-manifests";
              text = k8sModule.manifestsToYAML k8sModule.neutronStubManifests;
            };

            service-profile = pkgs.writeTextFile {
              name = "spectre-proxy-service-profile";
              text = k8sModule.manifestsToYAML k8sModule.serviceProfileManifests;
            };

            # Default package
            default = spectre-proxy;
          };

        # Apps for deployment and operations
        apps =
          let
            mkDeployApp =
              env:
              let
                manifestsPackage = self.packages.${system}."kubernetes-manifests-${env}";
                imagePackage = self.packages.${system}.spectre-proxy-image;
              in
              {
                type = "app";
                program = toString (
                  pkgs.writeShellScript "deploy-${env}" ''
                    set -euo pipefail

                    echo "🚀 Deploying SPECTRE to ${env} environment"
                    echo ""

                    # Check kubectl connectivity
                    if ! ${pkgs.kubectl}/bin/kubectl cluster-info &>/dev/null; then
                      echo "❌ Error: Cannot connect to Kubernetes cluster"
                      echo "   Make sure kubectl is configured and cluster is accessible"
                      exit 1
                    fi

                    # Load container image
                    if command -v kind &>/dev/null && kind get clusters 2>/dev/null | grep -q spectre; then
                      echo "📦 Loading container image to kind cluster..."
                      ${pkgs.docker}/bin/docker load < ${imagePackage}
                      kind load docker-image ${imagePackage.imageName}:${imagePackage.imageTag} --name spectre-dev
                    elif command -v docker &>/dev/null; then
                      echo "📦 Loading container image to Docker daemon..."
                      ${pkgs.skopeo}/bin/skopeo copy \
                        docker-archive:${imagePackage} \
                        docker-daemon:${imagePackage.imageName}:${imagePackage.imageTag}
                    fi

                    # Apply Kubernetes manifests
                    echo "☸️  Applying Kubernetes manifests..."
                    ${pkgs.kubectl}/bin/kubectl apply -f ${manifestsPackage}

                    echo ""
                    echo "✅ Deployment complete!"
                    echo ""
                    echo "Check status with:"
                    echo "  kubectl get pods -l app.kubernetes.io/name=spectre-proxy"
                    echo "  kubectl logs -l app.kubernetes.io/name=spectre-proxy -f"
                  ''
                );
              };
          in
          {
            # Deployment apps
            deploy-dev = mkDeployApp "dev";
            deploy-prod = mkDeployApp "prod";

            # Generate and display manifests
            show-manifests-dev = {
              type = "app";
              program = toString (
                pkgs.writeShellScript "show-manifests-dev" ''
                  ${pkgs.coreutils}/bin/cat ${self.packages.${system}.kubernetes-manifests-dev}
                ''
              );
            };

            show-manifests-prod = {
              type = "app";
              program = toString (
                pkgs.writeShellScript "show-manifests-prod" ''
                  ${pkgs.coreutils}/bin/cat ${self.packages.${system}.kubernetes-manifests-prod}
                ''
              );
            };

            # Run NATS server (dev)
            nats = {
              type = "app";
              program = "${self.packages.${system}.nats-server-dev}/bin/nats-server-spectre";
            };

            # Load image to Docker daemon
            load-image = {
              type = "app";
              program = toString (
                pkgs.writeShellScript "load-image" ''
                  set -euo pipefail
                  IMAGE=${self.packages.${system}.spectre-proxy-image}
                  echo "Loading image: $IMAGE"
                  ${pkgs.docker}/bin/docker load < $IMAGE
                  echo "✅ Image loaded successfully"
                ''
              );
            };
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
