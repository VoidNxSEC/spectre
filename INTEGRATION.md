# SPECTRE Integration Guide

**How to integrate external domain services with the SPECTRE framework**

---

## 🎯 Overview

SPECTRE follows a **hybrid architecture**:
- **Core infrastructure** (this repo): Event bus, proxy, secrets, observability
- **Domain services** (separate repos): Your specialized services

**Integration Method**: Event-driven communication via **NATS message bus**

---

## 🚀 Quick Integration (3 Steps)

### Step 1: Add SPECTRE Dependencies

Add to your service's `Cargo.toml`:

```toml
[dependencies]
spectre-core = { git = "https://github.com/kernelcore/spectre", branch = "main" }
spectre-events = { git = "https://github.com/kernelcore/spectre", branch = "main" }
tokio = { version = "1.35", features = ["full"] }
async-nats = "0.33"
```

### Step 2: Connect to NATS

```rust
use spectre_events::EventBus;
use spectre_core::{ServiceId, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Connect to NATS (Docker Compose default)
    let bus = EventBus::connect("nats://localhost:4222").await?;

    // Your service ID
    let service_id = ServiceId::new("my-service");

    println!("✅ Connected to SPECTRE event bus");
    Ok(())
}
```

### Step 3: Publish or Subscribe

**Publishing Events:**
```rust
use spectre_events::{Event, EventType};
use serde_json::json;

let event = Event::new(
    EventType::SystemMetrics,
    service_id.clone(),
    json!({ "cpu": 45.2, "memory_mb": 2048 }),
);

bus.publish(&event).await?;
```

**Subscribing to Events:**
```rust
use spectre_events::{Subscriber, EventHandler};

struct MyHandler;

#[async_trait::async_trait]
impl EventHandler for MyHandler {
    async fn handle(&self, event: Event) -> Result<()> {
        println!("Received: {:?}", event);
        Ok(())
    }
}

let nats_sub = bus.subscribe("system.metrics.v1").await?;
let mut subscriber = Subscriber::new(nats_sub, "system.metrics.v1");

subscriber.listen(MyHandler).await?;
```

---

## 📚 Integration Patterns

### Pattern 1: Request-Reply (Synchronous RPC)

**Use Case**: LLM gateway receiving inference requests

```rust
use std::time::Duration;

// Subscriber: Service handling requests
let mut sub = bus.subscribe("llm.request.v1").await?;

while let Some(msg) = sub.next().await {
    let request: Event = serde_json::from_slice(&msg.payload)?;

    // Process request
    let response = handle_llm_request(request).await?;

    // Reply to sender
    if let Some(reply_subject) = msg.reply {
        bus.client.publish(reply_subject, response.to_json()?.into()).await?;
    }
}

// Publisher: Client making request
let request = Event::new(EventType::LlmRequest, service_id, payload);
let response = bus.request(&request, Duration::from_secs(30)).await?;
```

### Pattern 2: Pub/Sub (Asynchronous Events)

**Use Case**: System monitoring broadcasting metrics

```rust
// Publisher: ai-agent-os
loop {
    let metrics = collect_system_metrics();
    let event = Event::new(EventType::SystemMetrics, service_id.clone(), metrics);
    bus.publish(&event).await?;

    tokio::time::sleep(Duration::from_secs(10)).await;
}

// Subscribers: spectre-observability, dashboard, alerts
// All receive the same event simultaneously
```

### Pattern 3: Queue Groups (Load Balancing)

**Use Case**: Multiple ML inference workers sharing load

```rust
// Worker 1, 2, 3 all subscribe to same queue group
let nats_sub = bus.subscribe_queue("inference.request.v1", "ml-workers").await?;

// NATS automatically load balances requests across workers
let mut subscriber = Subscriber::new(nats_sub, "inference.request.v1");
subscriber.listen(InferenceHandler).await?;
```

---

## 🎯 Event Schema Catalog

All events follow: `<category>.<action>.v<version>`

### Implemented Event Types

| Subject | Publisher | Subscribers | Payload |
|---------|-----------|-------------|---------|
| `llm.request.v1` | spectre-proxy | llm-gateway | `{ prompt, model, max_tokens }` |
| `llm.response.v1` | llm-gateway | spectre-proxy | `{ response, tokens, cost }` |
| `inference.request.v1` | spectre-proxy | ml-offload-api | `{ model, input }` |
| `vram.status.v1` | ml-offload-api | spectre-observability | `{ free_mb, used_mb, gpu_count }` |
| `system.metrics.v1` | ai-agent-os | spectre-observability | `{ cpu, memory, disk }` |
| `rag.query.v1` | spectre-proxy | rag-service | `{ query, top_k }` |
| `cost.incurred.v1` | ALL | spectre-observability | `{ service, amount_usd, metadata }` |

---

## 🔧 Real-World Integration Examples

### Example 1: securellm-bridge → SPECTRE

**Current Architecture**: Axum HTTP server with provider routing

**Integration Strategy**:
1. Keep Axum server for backward compatibility
2. Add NATS event publishing on every LLM request
3. Subscribe to config change events for dynamic reconfiguration

```rust
// In securellm-bridge/src/main.rs
use spectre_events::{EventBus, Event, EventType};

#[tokio::main]
async fn main() -> Result<()> {
    // Existing HTTP server
    let app = Router::new()
        .route("/v1/chat/completions", post(handle_completion));

    // NEW: Connect to SPECTRE
    let bus = EventBus::connect("nats://localhost:4222").await?;

    // NEW: Publish events
    async fn handle_completion(
        State(state): State<AppState>,
        Json(req): Json<ChatRequest>,
    ) -> Result<Json<ChatResponse>> {
        // Publish request event
        state.bus.publish(&Event::new(
            EventType::LlmRequest,
            ServiceId::new("securellm-bridge"),
            serde_json::to_value(&req)?,
        )).await?;

        // Existing processing
        let response = state.provider.complete(req).await?;

        // Publish response + cost event
        state.bus.publish(&Event::new(
            EventType::CostIncurred,
            ServiceId::new("securellm-bridge"),
            json!({ "provider": "openai", "cost_usd": 0.05 }),
        )).await?;

        Ok(Json(response))
    }

    // Serve
    axum::Server::bind(&"0.0.0.0:8080".parse()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
```

### Example 2: ml-offload-api → SPECTRE

**Current Architecture**: Axum + VRAM monitoring + model registry

**Integration Strategy**:
1. Publish VRAM status every 10 seconds
2. Subscribe to inference requests
3. Emit cost events for local inference

```rust
// In ml-offload-api/src/main.rs
use spectre_events::{EventBus, Event, EventType};

#[tokio::main]
async fn main() -> Result<()> {
    let bus = EventBus::connect("nats://localhost:4222").await?;
    let service_id = ServiceId::new("ml-offload-api");

    // Background task: Publish VRAM status
    let bus_clone = bus.clone();
    let service_id_clone = service_id.clone();
    tokio::spawn(async move {
        loop {
            let vram = monitor_vram().await;
            bus_clone.publish(&Event::new(
                EventType::VramStatus,
                service_id_clone.clone(),
                json!(vram),
            )).await.ok();

            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });

    // Subscribe to inference requests
    let mut sub = bus.subscribe("inference.request.v1").await?;

    while let Some(msg) = sub.next().await {
        let request: Event = serde_json::from_slice(&msg.payload)?;

        // Process inference
        let result = run_inference(request.payload).await?;

        // Emit cost (local inference = $0.00)
        bus.publish(&Event::new(
            EventType::CostIncurred,
            service_id.clone(),
            json!({ "model": "llama3", "cost_usd": 0.0 }),
        )).await?;

        // Reply
        if let Some(reply) = msg.reply {
            bus.client.publish(reply, result.to_json()?.into()).await?;
        }
    }

    Ok(())
}
```

### Example 3: ragtex (Python) → SPECTRE

**Current Architecture**: Python with LangChain + Vertex AI

**Integration Strategy**:
1. Use Python NATS client (`nats-py`)
2. Subscribe to RAG query events
3. Publish indexed document events

```python
# In ragtex/main.py
import asyncio
import json
from nats.aio.client import Client as NATS

async def main():
    nc = await NATS().connect("nats://localhost:4222")

    async def rag_query_handler(msg):
        request = json.loads(msg.data.decode())

        # Existing RAG pipeline
        results = await vector_search(request["query"])
        response = await generate_response(results)

        # Reply
        await nc.publish(msg.reply, json.dumps(response).encode())

    # Subscribe
    await nc.subscribe("rag.query.v1", cb=rag_query_handler)

    print("✅ RAG service connected to SPECTRE")

    # Keep alive
    await asyncio.Event().wait()

if __name__ == "__main__":
    asyncio.run(main())
```

---

## 🏗️ Infrastructure Setup

### 1. Start NATS

```bash
# Via Docker Compose (recommended)
cd /path/to/spectre
docker-compose up -d nats

# Verify
curl http://localhost:8222/healthz
# Expected: "ok"
```

### 2. Configure Service

```bash
# Environment variable
export NATS_URL=nats://localhost:4222

# Or in service config file (e.g., config.toml)
[nats]
url = "nats://localhost:4222"
max_reconnect_attempts = 5
reconnect_delay_ms = 1000
```

### 3. Run Service

```bash
# Rust service
cargo run --release

# Python service
python main.py
```

---

## 🔍 Observability Integration

All services should emit:

1. **Startup Event**:
   ```rust
   bus.publish(&Event::new(
       EventType::ServiceStarted,
       service_id.clone(),
       json!({ "version": "1.0.0", "timestamp": Utc::now() }),
   )).await?;
   ```

2. **Health Check Events** (every 30s):
   ```rust
   bus.publish(&Event::new(
       EventType::ServiceHealthy,
       service_id.clone(),
       json!({ "status": "ok", "uptime_secs": 3600 }),
   )).await?;
   ```

3. **Error Events**:
   ```rust
   bus.publish(&Event::new(
       EventType::ServiceError,
       service_id.clone(),
       json!({ "error": err.to_string(), "severity": "high" }),
   )).await?;
   ```

4. **Cost Events** (after every billable operation):
   ```rust
   bus.publish(&Event::new(
       EventType::CostIncurred,
       service_id.clone(),
       json!({ "operation": "llm_request", "cost_usd": 0.05 }),
   )).await?;
   ```

---

## 🛡️ Security Best Practices

### 1. Use CorrelationId for Tracing

```rust
use spectre_core::CorrelationId;

// Generate on first request
let correlation_id = CorrelationId::generate();

// Attach to all events in request chain
let event = Event::new_with_correlation(
    EventType::LlmRequest,
    service_id,
    payload,
    correlation_id.clone(),
);
```

### 2. Validate Event Payloads

```rust
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct LlmRequestPayload {
    prompt: String,
    #[serde(default = "default_max_tokens")]
    max_tokens: u32,
}

fn default_max_tokens() -> u32 { 1000 }

// In handler
let payload: LlmRequestPayload = serde_json::from_value(event.payload)?;
if payload.prompt.is_empty() {
    return Err(SpectreError::InvalidPayload("Empty prompt".into()));
}
```

### 3. Handle Connection Failures

```rust
use std::time::Duration;

async fn connect_with_retry() -> Result<EventBus> {
    let mut attempts = 0;
    loop {
        match EventBus::connect("nats://localhost:4222").await {
            Ok(bus) => return Ok(bus),
            Err(e) if attempts < 5 => {
                attempts += 1;
                eprintln!("NATS connection failed (attempt {}): {}", attempts, e);
                tokio::time::sleep(Duration::from_secs(2_u64.pow(attempts))).await;
            }
            Err(e) => return Err(e),
        }
    }
}
```

---

## 📊 Testing Your Integration

### Unit Test: Event Publishing

```rust
#[tokio::test]
async fn test_publish_event() {
    let bus = EventBus::connect("nats://localhost:4222").await.unwrap();

    let event = Event::new(
        EventType::SystemMetrics,
        ServiceId::new("test-service"),
        json!({ "cpu": 50.0 }),
    );

    let result = bus.publish(&event).await;
    assert!(result.is_ok());
}
```

### Integration Test: Request-Reply

```rust
#[tokio::test]
async fn test_request_reply() {
    let bus = EventBus::connect("nats://localhost:4222").await.unwrap();

    // Start responder
    tokio::spawn(async move {
        let bus2 = EventBus::connect("nats://localhost:4222").await.unwrap();
        let mut sub = bus2.subscribe("test.request.v1").await.unwrap();

        while let Some(msg) = sub.next().await {
            let response = Event::new(/* ... */);
            bus2.client.publish(msg.reply.unwrap(), response.to_json().unwrap().into()).await.ok();
        }
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send request
    let request = Event::new(/* ... */);
    let response = bus.request(&request, Duration::from_secs(5)).await;

    assert!(response.is_ok());
}
```

---

## 🚀 Next Steps

1. **Validate Integration**: Run `cargo test` to ensure events publish/subscribe correctly
2. **Add Observability**: Emit health check and cost events
3. **Update Documentation**: Document your service's event schema
4. **Deploy**: Containerize and deploy alongside SPECTRE infrastructure

---

## 🆘 Troubleshooting

**Issue**: "Connection refused (os error 111)"
- **Fix**: Start NATS: `docker-compose up -d nats`

**Issue**: "Subscription timeout"
- **Fix**: Verify subject name matches exactly (case-sensitive)

**Issue**: "Events not received"
- **Fix**: Check queue group name (subscribers in same queue group load balance)

**Issue**: "High latency"
- **Fix**: Use request-reply only when needed; prefer pub/sub for async

---

**For Questions**: See `README.md`, `STATUS.md`, or `NEXT_STEPS.md`

**Last Updated**: 2026-01-09
