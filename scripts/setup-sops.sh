#!/usr/bin/env bash
# One-time SOPS bootstrapper for Spectre Fleet.
# Run once per developer machine.
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
info() { echo -e "${GREEN}[spectre-sops]${NC} $*"; }
warn() { echo -e "${YELLOW}[spectre-sops]${NC} $*"; }
die()  { echo -e "${RED}[spectre-sops] ERROR:${NC} $*" >&2; exit 1; }

# ── 1. Check tools ──────────────────────────────────────────────────────────
for tool in sops age-keygen age; do
  command -v "$tool" >/dev/null 2>&1 || die "$tool not found. Run: nix develop"
done

# ── 2. Check if voidnx-api already has an age key ───────────────────────────
AGE_KEY_FILE="${SOPS_AGE_KEY_FILE:-${XDG_CONFIG_HOME:-$HOME/.config}/sops/age/keys.txt}"

if [ -f "$AGE_KEY_FILE" ]; then
  warn "Age key already exists at $AGE_KEY_FILE — reusing."
else
  info "Generating new age key at $AGE_KEY_FILE ..."
  mkdir -p "$(dirname "$AGE_KEY_FILE")"
  age-keygen -o "$AGE_KEY_FILE"
  chmod 600 "$AGE_KEY_FILE"
  info "Key generated."
fi

PUBLIC_KEY=$(grep "^# public key:" "$AGE_KEY_FILE" | awk '{print $4}')
[ -n "$PUBLIC_KEY" ] || die "Could not extract public key from $AGE_KEY_FILE"
info "Public key: $PUBLIC_KEY"

# ── 3. Create .sops.yaml ───────────────────────────────────────────────────
cat > .sops.yaml <<YAMLEOF
creation_rules:
  - path_regex: secrets/.*\\.(env|json|yaml)\$
    age: >-
      $PUBLIC_KEY
YAMLEOF
info ".sops.yaml created."

# ── 4. Create initial encrypted dev secrets ─────────────────────────────────
SECRET_FILE="secrets/dev.enc.env"
if [ -f "$SECRET_FILE" ]; then
  warn "$SECRET_FILE already exists — skipping."
else
  info "Creating $SECRET_FILE ..."
  cat > /tmp/spectre_dev_template.env << 'EOF'
# Spectre Proxy — Development Secrets
CF_API_TOKEN=replace-me
CF_ACCOUNT_ID=replace-me
CF_ZONE_ID=replace-me
NATS_URL=nats://localhost:4222
JWT_SECRET=replace-me-with-32-char-random
DATABASE_URL=sqlite://data/spectre.db
NEO4J_URI=bolt://localhost:7687
NEO4J_USER=neo4j
NEO4J_PASSWORD=replace-me
EOF
  cp /tmp/spectre_dev_template.env secrets/dev.env
  sops -e -i secrets/dev.env
  mv secrets/dev.env "$SECRET_FILE"
  rm /tmp/spectre_dev_template.env
  info "$SECRET_FILE created."
fi

# ── 5. Done ─────────────────────────────────────────────────────────────────
echo ""
info "SOPS setup complete for Spectre Fleet."
echo ""
echo "  Add to your shell profile:"
echo "    export SOPS_AGE_KEY_FILE=\"$AGE_KEY_FILE\""
echo ""
echo "  Commands:"
echo "    sops secrets/dev.enc.env           — view/edit encrypted secrets"
echo "    sops -d secrets/dev.enc.env        — decrypt to stdout"
echo "    sops -e -i secrets/prod.enc.env    — encrypt a new file"
