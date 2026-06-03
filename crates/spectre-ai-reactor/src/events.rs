// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 VoidNXLabs
//
// AI Event Namespace — canonical subject hierarchy for cross-stack AI events.
//
// Phase 5 #50: Unified AI Event Namespace.
//
// Producer subjects (what other services publish):
//   ml_offload.inference.completed  — inference succeeded
//   ml_offload.inference.failed     — inference failed
//   ml_offload.queue.depth          — queue depth metric
//   neoland.pipeline.output.v1      — ADR pipeline result
//   sentinel.alert.v1               — compliance alert
//
// Spectre output subjects (what this reactor publishes):
//   spectre.ai.action.v1            — generic AI action
//   spectre.ai.scale.v1             — scale up/down inference
//   spectre.ai.rollback.v1          — rollback model version
//   spectre.ai.alert.v1             — enriched alert
//
// Event envelope (JSON, all subjects):
//   {
//     "source": "spectre-ai-reactor",
//     "ts": "2026-05-29T12:00:00Z",
//     "correlation_id": "uuid-v4",
//     "payload": { ... }
//   }

use chrono::Utc;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// AI Event Namespace Constants
// ─────────────────────────────────────────────────────────────────────────────

/// JetStream stream name for AI events (7 day retention).
pub const STREAM_NAME: &str = "SPECTRE_AI_EVENTS";

/// Producer subjects (published by other services, consumed by us).
pub mod producers {
    pub const INFERENCE_COMPLETED: &str = "ml_offload.inference.completed";
    pub const INFERENCE_FAILED: &str = "ml_offload.inference.failed";
    pub const QUEUE_DEPTH: &str = "ml_offload.queue.depth";
    pub const PIPELINE_OUTPUT: &str = "neoland.pipeline.output.v1";
    pub const SENTINEL_ALERT: &str = "sentinel.alert.v1";
}

/// Output subjects (published by spectre-ai-reactor).
pub mod actions {
    pub const AI_ACTION: &str = "spectre.ai.action.v1";
    pub const AI_SCALE: &str = "spectre.ai.scale.v1";
    pub const AI_ROLLBACK: &str = "spectre.ai.rollback.v1";
    pub const AI_ALERT: &str = "spectre.ai.alert.v1";
}

// ─────────────────────────────────────────────────────────────────────────────
// AI Event Types
// ─────────────────────────────────────────────────────────────────────────────

/// Standard envelope for all AI events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiEvent {
    /// Service that generated this event
    pub source: String,
    /// ISO 8601 timestamp
    pub ts: String,
    /// UUID v4 for tracing across services
    pub correlation_id: String,
    /// Event-specific payload
    pub payload: AiPayload,
}

/// Discriminated payload for different AI event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AiPayload {
    #[serde(rename = "inference.completed")]
    InferenceCompleted(InferenceCompletedPayload),

    #[serde(rename = "inference.failed")]
    InferenceFailed(InferenceFailedPayload),

    #[serde(rename = "queue.depth")]
    QueueDepth(QueueDepthPayload),

    #[serde(rename = "pipeline.output")]
    PipelineOutput(PipelineOutputPayload),

    #[serde(rename = "sentinel.alert")]
    SentinelAlert(SentinelAlertPayload),

    #[serde(rename = "scale")]
    Scale(ScaleActionPayload),

    #[serde(rename = "rollback")]
    Rollback(RollbackActionPayload),

    #[serde(rename = "alert")]
    Alert(AlertActionPayload),
}

/// Payload when inference completes successfully.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceCompletedPayload {
    pub model: String,
    pub model_version: String,
    pub latency_ms: u64,
    pub tokens_generated: u32,
    pub request_id: String,
}

/// Payload when inference fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceFailedPayload {
    pub model: String,
    pub model_version: String,
    pub error: String,
    pub error_type: String, // timeout, oom, model_load, etc.
    pub request_id: String,
    pub retry_count: u32,
}

/// Payload for queue depth metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueDepthPayload {
    pub queue_name: String,
    pub depth: u64,
    pub capacity: u64,
    pub avg_wait_ms: f64,
}

/// Payload from Neoland pipeline output (ADR).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineOutputPayload {
    pub session_id: String,
    pub decision: String,
    pub risk_level: RiskLevel,
    pub model: String,
    pub confidence: f64,
    pub reasoning: String,
}

/// Payload from Sentinel compliance alert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelAlertPayload {
    pub alert_id: String,
    pub severity: String,
    pub regulation: String,
    pub guardrail: String,
    pub details: String,
}

/// Scale action (publish to KEDA).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaleActionPayload {
    pub direction: ScaleDirection,
    pub target: String,
    pub reason: String,
    pub current_replicas: u32,
    pub target_replicas: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScaleDirection {
    #[serde(rename = "up")]
    Up,
    #[serde(rename = "down")]
    Down,
}

/// Rollback action (switch model version).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackActionPayload {
    pub model: String,
    pub from_version: String,
    pub to_version: String,
    pub reason: String,
    pub failure_count: u32,
}

/// Enriched alert action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertActionPayload {
    pub alert_id: String,
    pub severity: String,
    pub title: String,
    pub description: String,
    pub suggested_action: String,
    pub source_events: Vec<String>,
}

/// Risk level from Neoland ADRs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskLevel {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "critical")]
    Critical,
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper constructors
// ─────────────────────────────────────────────────────────────────────────────

impl AiEvent {
    pub fn new(source: &str, payload: AiPayload) -> Self {
        Self {
            source: source.to_string(),
            ts: Utc::now().to_rfc3339(),
            correlation_id: uuid::Uuid::new_v4().to_string(),
            payload,
        }
    }
}
