# NATS Failure Scenarios

Reference for SPECTRE operators and developers. Covers expected behavior when NATS encounters failures, and how the EventBus handles each scenario.

## Architecture Context

```
spectre-proxy ──► EventBus (async_nats) ──► NATS Server ──► subscribers
                  └── retry_on_initial_connect
                  └── reconnect_delay_callback (linear backoff)
                  └── flush-on-connect (handshake guarantee)
```

## Failure Scenarios

### 1. NATS Server Not Available at Startup

**Trigger**: Proxy starts before NATS is ready.

**Behavior**: `retry_on_initial_connect()` is enabled — the client will retry connecting indefinitely in the background. The `flush()` call after connect blocks until the handshake completes, so `EventBus::connect()` will await until NATS is reachable.

**Impact**: Proxy startup blocks until NATS is available. The `/health` endpoint is unreachable during this time.

**Mitigation**: Deploy NATS before the proxy (K8s init containers or dependency ordering). The startup probe (`failureThreshold=30, periodSeconds=2`) gives 60s for NATS to come up.

### 2. NATS Server Restarts (Transient Disconnect)

**Trigger**: NATS pod restart, rolling update, or brief network partition.

**Behavior**: `async_nats` fires `Event::Disconnected`, then automatically reconnects using the `reconnect_delay_callback` (linear backoff: 1s, 2s, 3s, ... up to 10 attempts). On reconnect, `Event::Connected` fires. Subscriptions are automatically re-established by the client library.

**Impact**: Messages published during disconnection are **lost** (NATS core pub/sub has no persistence). The proxy continues operating — requests to `/health` return OK, but event publishing silently fails.

**Mitigation**: For critical events, use JetStream (already configured) which provides at-least-once delivery with server-side persistence. Non-critical telemetry events can tolerate loss.

### 3. NATS Server Permanently Down

**Trigger**: NATS server crashes and doesn't restart. All reconnect attempts exhausted (10 attempts with linear backoff ≈ 55s total).

**Behavior**: Client enters `Disconnected` state permanently. `is_connected()` returns `false`. The `/ready` endpoint returns `503 Service Unavailable` because the readiness check probes NATS connectivity. Kubernetes marks the pod as not-ready and stops routing traffic.

**Impact**: Proxy stops receiving traffic via the Service. Existing in-flight HTTP requests (proxy→neutron) continue to completion. Event publishing fails silently.

**Mitigation**: NATS should run as a cluster (3+ nodes) so single-node failure doesn't cause total outage. Monitor `/ready` with alerting.

### 4. Slow NATS (High Latency)

**Trigger**: Network congestion, overloaded NATS server, or disk I/O saturation (JetStream).

**Behavior**: `publish()` calls take longer but eventually succeed. No timeout is set on publish operations in the current implementation. The proxy's HTTP response time is **not affected** — event publishing is fire-and-forget and doesn't block the HTTP response.

**Impact**: Minimal. Event delivery is delayed but the proxy's HTTP path is decoupled from NATS latency.

**Mitigation**: Monitor NATS server metrics (`nats-server --monitor`, port 8222). Set JetStream store limits to prevent disk pressure.

### 5. Message Backpressure (Slow Consumer)

**Trigger**: A subscriber can't keep up with the publish rate.

**Behavior**: NATS core subjects: messages are **dropped** for slow consumers after the pending buffer fills (default 64KB or 65536 messages). JetStream: messages are persisted server-side and delivered when the consumer catches up.

**Impact**: Telemetry events may be lost. Workflow-critical events (via JetStream) are preserved.

**Mitigation**: Use queue groups for load-balanced consumption (`queue_subscribe`). Monitor consumer pending counts via NATS monitoring.

### 6. Network Partition (Split Brain)

**Trigger**: Network split between proxy and NATS, or between NATS cluster nodes.

**Behavior**: The client sees a disconnect and attempts reconnection. In a NATS cluster, the cluster self-heals when the partition resolves. Messages published to the reachable side are delivered; messages to the unreachable side are lost (core) or queued (JetStream).

**Impact**: Depends on which side of the partition the proxy is on. If proxy can't reach any NATS node, same as scenario 3.

**Mitigation**: Deploy NATS nodes across availability zones. Use JetStream replication factor > 1.

### 7. Invalid/Corrupted Messages

**Trigger**: Publisher sends malformed JSON, or binary corruption in transit.

**Behavior**: The `Event::from_json()` deserialization fails on the subscriber side. The `EventHandler::handle()` is never called. The error is logged but the subscriber continues processing subsequent messages.

**Impact**: Single corrupted message is skipped. No crash or subscriber death.

**Mitigation**: Event validation happens at publish time (`event.to_json()` serializes before sending). Binary corruption is detected by NATS protocol checksums.

## Recovery Matrix

| Scenario | Detection | Auto-Recovery | Data Loss | Proxy Impact |
|----------|-----------|---------------|-----------|--------------|
| NATS down at startup | Startup blocks | Yes (retry) | None | Delayed start |
| Transient disconnect | `Event::Disconnected` | Yes (reconnect) | Core: yes, JS: no | None (fire-and-forget) |
| Permanent down | `/ready` → 503 | No | Yes | K8s stops routing |
| High latency | Monitoring | N/A | No | None (decoupled) |
| Slow consumer | NATS metrics | Backpressure | Core: yes, JS: no | None |
| Network partition | Disconnect event | Cluster heal | Partial | Depends on side |
| Corrupted messages | Deserialization error | Skip message | Single msg | None |

## Monitoring Checklist

- `/ready` endpoint: NATS connectivity check (K8s readiness probe)
- NATS monitoring endpoint (`:8222/varz`): connections, messages, bytes
- JetStream metrics (`:8222/jsz`): stream/consumer lag, pending messages
- Application logs: `NATS connected`, `NATS disconnected` events
- Prometheus: `spectre_events_published_total` counter for publish rate
