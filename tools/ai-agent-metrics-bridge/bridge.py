import asyncio
import json
import os
import signal
import ssl
from typing import Any

import nats
from prometheus_client import Counter, Gauge, start_http_server


NATS_URL = os.getenv("NATS_URL", "nats://nats:4222")
NATS_NKEY_SEED = os.getenv("NATS_NKEY_SEED", "").strip()
NATS_NKEY_SEED_FILE = os.getenv("NATS_NKEY_SEED_FILE", "").strip()
NATS_CA_FILE = os.getenv("NATS_CA_FILE", "").strip()
NATS_CLIENT_CERT_FILE = os.getenv("NATS_CLIENT_CERT_FILE", "").strip()
NATS_CLIENT_KEY_FILE = os.getenv("NATS_CLIENT_KEY_FILE", "").strip()
BRIDGE_PORT = int(os.getenv("BRIDGE_PORT", "9101"))
SUBJECT = os.getenv("AI_AGENT_METRICS_SUBJECT", "system.metrics.v1")


CPU_PERCENT = Gauge("ai_agent_cpu_percent", "Latest ai-agent-os CPU usage percent")
MEMORY_PERCENT = Gauge("ai_agent_memory_percent", "Latest ai-agent-os memory usage percent")
MEMORY_USED_BYTES = Gauge("ai_agent_memory_used_bytes", "Latest ai-agent-os used memory bytes")
MEMORY_TOTAL_BYTES = Gauge("ai_agent_memory_total_bytes", "Latest ai-agent-os total memory bytes")
TEMP_AVG_CELSIUS = Gauge("ai_agent_temp_avg_celsius", "Latest ai-agent-os average temperature")
TEMP_MAX_CELSIUS = Gauge("ai_agent_temp_max_celsius", "Latest ai-agent-os maximum temperature")
DISK_COUNT = Gauge("ai_agent_disk_count", "Latest ai-agent-os disk count")
NET_RX_BYTES_TOTAL = Gauge("ai_agent_net_rx_bytes_total", "Latest ai-agent-os received bytes")
NET_TX_BYTES_TOTAL = Gauge("ai_agent_net_tx_bytes_total", "Latest ai-agent-os transmitted bytes")
LAST_EVENT_TS = Gauge("ai_agent_last_event_timestamp_seconds", "Unix timestamp of the last ai-agent-os metrics event")
PUBLISH_TOTAL = Counter("ai_agent_publish_total", "Total ai-agent-os metrics events bridged into Prometheus")


def _payload_from_message(raw: bytes) -> dict[str, Any]:
    data = json.loads(raw.decode())
    if isinstance(data, dict) and isinstance(data.get("payload"), dict):
        return data["payload"]
    if isinstance(data, dict):
        return data
    raise ValueError("Unsupported message payload")


def _set_gauge(gauge: Gauge, payload: dict[str, Any], key: str) -> None:
    value = payload.get(key)
    if isinstance(value, (int, float)):
        gauge.set(value)


async def run() -> None:
    stop_event = asyncio.Event()

    def _stop(*_: Any) -> None:
        stop_event.set()

    signal.signal(signal.SIGTERM, _stop)
    signal.signal(signal.SIGINT, _stop)

    start_http_server(BRIDGE_PORT)

    connect_kwargs: dict[str, Any] = {
        "servers": [NATS_URL],
        "connect_timeout": 5,
        "allow_reconnect": True,
        "max_reconnect_attempts": -1,
    }
    nkey_seed = NATS_NKEY_SEED
    if not nkey_seed and NATS_NKEY_SEED_FILE and os.path.exists(NATS_NKEY_SEED_FILE):
        with open(NATS_NKEY_SEED_FILE, "r", encoding="utf-8") as handle:
            for line in handle:
                candidate = line.strip()
                if candidate and not candidate.startswith("#"):
                    nkey_seed = candidate
                    break
    if nkey_seed:
        connect_kwargs["nkeys_seed_str"] = nkey_seed
    if NATS_URL.startswith("tls://") or NATS_CA_FILE or NATS_CLIENT_CERT_FILE or NATS_CLIENT_KEY_FILE:
        tls_context = ssl.create_default_context(cafile=NATS_CA_FILE or None)
        if NATS_CLIENT_CERT_FILE and NATS_CLIENT_KEY_FILE:
            tls_context.load_cert_chain(
                certfile=NATS_CLIENT_CERT_FILE,
                keyfile=NATS_CLIENT_KEY_FILE,
            )
        connect_kwargs["tls"] = tls_context

    nc = await nats.connect(**connect_kwargs)

    async def handler(msg: Any) -> None:
        payload = _payload_from_message(msg.data)
        _set_gauge(CPU_PERCENT, payload, "cpu_percent")
        _set_gauge(MEMORY_PERCENT, payload, "memory_percent")
        _set_gauge(MEMORY_USED_BYTES, payload, "memory_used_bytes")
        _set_gauge(MEMORY_TOTAL_BYTES, payload, "memory_total_bytes")
        _set_gauge(TEMP_AVG_CELSIUS, payload, "temp_avg_celsius")
        _set_gauge(TEMP_MAX_CELSIUS, payload, "temp_max_celsius")
        _set_gauge(DISK_COUNT, payload, "disk_count")
        _set_gauge(NET_RX_BYTES_TOTAL, payload, "net_rx_bytes")
        _set_gauge(NET_TX_BYTES_TOTAL, payload, "net_tx_bytes")
        LAST_EVENT_TS.set_to_current_time()
        PUBLISH_TOTAL.inc()

    sub = await nc.subscribe(SUBJECT, cb=handler)

    await stop_event.wait()

    await sub.unsubscribe()
    await nc.drain()


if __name__ == "__main__":
    asyncio.run(run())
