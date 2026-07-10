# cerebro-rag — Spectre Fleet Domain Service

Cerebro is the RAG/knowledge-extraction domain service of the Spectre Fleet.
It is implemented in **Python** (not Rust) and lives at `~/master/cerebro`.

## Integration contract

| Item | Value |
|------|-------|
| Service ID | `cerebro-rag` |
| Ingest subject | `rag.index.v1` (JetStream, stream `CEREBRO_INGEST`) |
| Completion subject | `document.indexed.v1.<correlation_id>` (core NATS) |
| Query subject | `rag.query.v1` (future) |
| OTLP | via `OTEL_EXPORTER_OTLP_ENDPOINT` env var |

## Repository

`~/master/cerebro` — `src/cerebro/nats/` contains the Spectre-conformant NATS client.

## Running

```bash
# Start the ingest worker (connects to NATS_URL)
cd ~/master/cerebro
nix develop --command poetry run python -m cerebro.nats.worker

# Or via Helm (k3s / BREV)
helm upgrade --install cerebro charts/cerebro -f charts/cerebro/values-prod.yaml
```

## Helm chart

`~/master/cerebro/charts/cerebro/` — deploys `cerebro-api`, `cerebro-ingest-worker`,
`cerebro-reranker`, and optionally a NATS JetStream sidecar.
