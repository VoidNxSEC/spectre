# Comandos para Validar Tudo

## 1. Build Completo
```bash
nix develop --command cargo build --workspace --release
```

## 2. Rodar Testes Unitários
```bash
nix develop --command cargo test --lib --workspace
```

## 3. Subir Infraestrutura
```bash
docker-compose up -d
```

## 4. Rodar Testes de Integração
```bash
nix develop --command cargo test --test test_event_bus
```

## 5. Build Docker Image
```bash
docker build -t spectre-proxy:latest .
```

## 6. Rodar Proxy
```bash
# Desenvolvimento (sem TLS)
export JWT_SECRET="dev-secret-change-me"
export NATS_URL="nats://localhost:4222"
export NEUTRON_URL="http://localhost:8000"
nix develop --command cargo run -p spectre-proxy
```

## 7. Testar Endpoints

### Health Check (sem auth)
```bash
curl http://localhost:3000/health
# Esperado: OK
```

### Ready Check (sem auth)
```bash
curl http://localhost:3000/ready
# Esperado: {"status":"ready","nats":true,"upstream":false}
```

### Metrics (sem auth)
```bash
curl http://localhost:3000/metrics
# Esperado: formato Prometheus
```

### API Protegida (com auth)
```bash
# Gerar token JWT válido primeiro
# Exemplo hardcoded para dev:
TOKEN="eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ0ZXN0LXVzZXIiLCJyb2xlIjoic2VydmljZSIsImV4cCI6OTk5OTk5OTk5OX0.xxx"

curl -H "Authorization: Bearer $TOKEN" \
     -X POST \
     http://localhost:3000/api/v1/ingest \
     -d '{"test": "data"}'
```

## 8. Lint
```bash
nix develop --command cargo clippy --all-targets -- -D warnings
```

## 9. Format
```bash
nix develop --command cargo fmt --check
```

## 10. Audit
```bash
nix develop --command cargo audit
```

## 11. Ver Status Git
```bash
git status
git diff
```

---

## 🔥 Script de Validação Completa

```bash
#!/bin/bash
set -e

echo "==> Build workspace"
nix develop --command cargo build --workspace --release

echo "==> Unit tests"
nix develop --command cargo test --lib --workspace

echo "==> Subir infra"
docker-compose up -d
sleep 5

echo "==> Integration tests"
nix develop --command cargo test --test test_event_bus -- --test-threads=1

echo "==> Build Docker"
docker build -t spectre-proxy:latest .

echo "==> Clippy"
nix develop --command cargo clippy --all-targets -- -D warnings

echo "==> Format check"
nix develop --command cargo fmt --check

echo "✅ TUDO OK!"
```

Salve como `scripts/validate-all.sh` e rode com `chmod +x scripts/validate-all.sh && ./scripts/validate-all.sh`
