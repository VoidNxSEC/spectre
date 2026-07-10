// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 VoidNXLabs
//
// Reasoning Layer — deterministic rule engine for AI event reactions.
//
// Phase 5 #52: Rules-based reactor that decides what action to emit.
//
// Design:
//   - Pure functions: each rule is (Event, State) → Option<Action>
//   - Thresholds loaded from config (NixOS module options)
//   - Extension point for LLM-assisted decisions (Phase 6)

use crate::events::*;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Threshold configuration for reasoning rules.
#[derive(Debug, Clone)]
pub struct ReasoningConfig {
    /// Max consecutive inference failures before triggering rollback
    pub max_consecutive_failures: u32,
    /// Queue depth threshold (% of capacity) to trigger scale-up
    pub queue_depth_threshold_pct: f64,
    /// Seconds of sustained high queue before scale-up
    pub queue_sustain_seconds: u64,
    /// Min seconds between scale actions (avoid flapping)
    pub scale_cooldown_seconds: u64,
    /// Model to rollback TO (last known stable)
    pub stable_model_version: String,
}

impl Default for ReasoningConfig {
    fn default() -> Self {
        Self {
            max_consecutive_failures: 5,
            queue_depth_threshold_pct: 0.8,
            queue_sustain_seconds: 30,
            scale_cooldown_seconds: 60,
            stable_model_version: "latest-stable".to_string(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Reasoning Engine
// ─────────────────────────────────────────────────────────────────────────────

/// State tracked by the reasoning engine across events.
#[derive(Debug, Default)]
pub struct ReactorState {
    /// Consecutive inference failures (reset on success)
    pub consecutive_failures: AtomicU32,
    /// Current inference queue depth
    pub queue_depth: AtomicU64,
    /// Queue capacity
    pub queue_capacity: AtomicU64,
    /// Current replica count
    pub current_replicas: AtomicU32,
    /// Timestamp of last scale action (unix seconds)
    pub last_scale_action: AtomicU64,
    /// Circuit breaker is open?
    pub circuit_breaker_open: AtomicU32, // 0 = closed, 1 = open
    /// Last model to successfully complete inference
    pub last_successful_model: parking_lot::Mutex<String>,
}

impl ReactorState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// The reasoning engine — consumes AI events and emits reactive actions.
pub struct ReasoningEngine {
    config: ReasoningConfig,
    state: Arc<ReactorState>,
}

impl ReasoningEngine {
    pub fn new(config: ReasoningConfig) -> Self {
        Self {
            config,
            state: Arc::new(ReactorState::new()),
        }
    }

    pub fn state(&self) -> &Arc<ReactorState> {
        &self.state
    }

    // ── Rule 1: Inference failed → check rollback threshold ─────────────

    /// Process an inference failure.
    /// Returns a rollback action if consecutive failures exceed threshold.
    pub fn on_inference_failed(&self, payload: &InferenceFailedPayload) -> Option<AiEvent> {
        let failures = self
            .state
            .consecutive_failures
            .fetch_add(1, Ordering::SeqCst)
            + 1;

        tracing::warn!(
            consecutive_failures = failures,
            model = %payload.model,
            error = %payload.error,
            "Inference failure detected"
        );

        if failures >= self.config.max_consecutive_failures {
            tracing::error!(
                threshold = self.config.max_consecutive_failures,
                "Rollback threshold reached — emitting rollback action"
            );

            Some(AiEvent::new(
                "spectre-ai-reactor",
                AiPayload::Rollback(RollbackActionPayload {
                    model: payload.model.clone(),
                    from_version: payload.model_version.clone(),
                    to_version: self.config.stable_model_version.clone(),
                    reason: format!(
                        "{} consecutive inference failures. Last error: {}",
                        failures, payload.error
                    ),
                    failure_count: failures,
                }),
            ))
        } else {
            None
        }
    }

    // ── Rule 2: Inference succeeded → reset failure counter ──────────────

    /// Reset failure counter on success.
    pub fn on_inference_completed(&self, payload: &InferenceCompletedPayload) -> Option<AiEvent> {
        self.state.consecutive_failures.store(0, Ordering::SeqCst);
        *self.state.last_successful_model.lock() = payload.model_version.clone();

        tracing::debug!(
            model = %payload.model,
            version = %payload.model_version,
            latency = payload.latency_ms,
            "Inference completed — failure counter reset"
        );

        None // No action needed on success
    }

    // ── Rule 3: Queue depth threshold → scale up ─────────────────────────

    /// Check if queue depth warrants scaling.
    pub fn on_queue_depth(&self, payload: &QueueDepthPayload) -> Option<AiEvent> {
        self.state
            .queue_depth
            .store(payload.depth, Ordering::SeqCst);
        self.state
            .queue_capacity
            .store(payload.capacity, Ordering::SeqCst);

        let ratio = payload.depth as f64 / payload.capacity as f64;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let last_action = self.state.last_scale_action.load(Ordering::SeqCst);

        // Check cooldown
        if now - last_action < self.config.scale_cooldown_seconds {
            return None;
        }

        if ratio >= self.config.queue_depth_threshold_pct {
            let current = self.state.current_replicas.load(Ordering::SeqCst);
            let target = (current as f64 * 1.5).ceil() as u32;
            let target = target.max(current + 1);

            self.state.last_scale_action.store(now, Ordering::SeqCst);

            tracing::info!(
                queue_depth = payload.depth,
                ratio = format!("{:.1}%", ratio * 100.0),
                current_replicas = current,
                target_replicas = target,
                "Queue threshold exceeded — emitting scale-up"
            );

            Some(AiEvent::new(
                "spectre-ai-reactor",
                AiPayload::Scale(ScaleActionPayload {
                    direction: ScaleDirection::Up,
                    target: payload.queue_name.clone(),
                    reason: format!("Queue depth {:.1}% of capacity", ratio * 100.0),
                    current_replicas: current,
                    target_replicas: target,
                }),
            ))
        } else {
            None
        }
    }

    // ── Rule 4: Neoland critical risk → alert ────────────────────────────

    /// React to Neoland pipeline output with critical risk.
    pub fn on_pipeline_output(&self, payload: &PipelineOutputPayload) -> Option<AiEvent> {
        if payload.risk_level == RiskLevel::Critical {
            tracing::warn!(
                session_id = %payload.session_id,
                decision = %payload.decision,
                risk = "critical",
                "Neoland flagged critical risk — emitting alert"
            );

            Some(AiEvent::new(
                "spectre-ai-reactor",
                AiPayload::Alert(AlertActionPayload {
                    alert_id: uuid::Uuid::new_v4().to_string(),
                    severity: "critical".to_string(),
                    title: format!("Critical Risk: {}", payload.decision),
                    description: payload.reasoning.clone(),
                    suggested_action: "Review pipeline output and consider rollback".to_string(),
                    source_events: vec![format!("neoland.session.{}", payload.session_id)],
                }),
            ))
        } else {
            None
        }
    }

    // ── Rule 5: Sentinel alert relay ─────────────────────────────────────

    /// Relay Sentinel compliance alerts with enriched context.
    pub fn on_sentinel_alert(&self, payload: &SentinelAlertPayload) -> Option<AiEvent> {
        let severity = match payload.severity.as_str() {
            "block" | "critical" => "critical",
            "high" => "high",
            _ => "medium",
        };

        tracing::warn!(
            alert_id = %payload.alert_id,
            severity = %severity,
            regulation = %payload.regulation,
            "Relaying Sentinel alert"
        );

        Some(AiEvent::new(
            "spectre-ai-reactor",
            AiPayload::Alert(AlertActionPayload {
                alert_id: payload.alert_id.clone(),
                severity: severity.to_string(),
                title: format!(
                    "Compliance Alert: {} ({})",
                    payload.guardrail, payload.regulation
                ),
                description: payload.details.clone(),
                suggested_action: "Review compliance violation and take corrective action"
                    .to_string(),
                source_events: vec![format!("sentinel.alert.{}", payload.alert_id)],
            }),
        ))
    }

    // ── Rule 6: Recovery → scale down ────────────────────────────────────

    /// After failures recover, scale back down.
    pub fn on_recovery(&self) -> Option<AiEvent> {
        let failures = self.state.consecutive_failures.load(Ordering::SeqCst);
        if failures == 0 {
            let current = self.state.current_replicas.load(Ordering::SeqCst);
            // Only scale down if we scaled up previously
            if current > 1 {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let last_action = self.state.last_scale_action.load(Ordering::SeqCst);

                if now - last_action >= self.config.scale_cooldown_seconds {
                    self.state.last_scale_action.store(now, Ordering::SeqCst);

                    tracing::info!(
                        current_replicas = current,
                        "System recovered — emitting scale-down"
                    );

                    return Some(AiEvent::new(
                        "spectre-ai-reactor",
                        AiPayload::Scale(ScaleActionPayload {
                            direction: ScaleDirection::Down,
                            target: "llama-server".to_string(),
                            reason: "System recovered — scaling back down".to_string(),
                            current_replicas: current,
                            target_replicas: current - 1,
                        }),
                    ));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_failures_reset_on_success() {
        let config = ReasoningConfig::default();
        let engine = ReasoningEngine::new(config);

        // Simulate 3 failures
        for _ in 0..3 {
            engine.on_inference_failed(&InferenceFailedPayload {
                model: "llama3".into(),
                model_version: "v1.0".into(),
                error: "timeout".into(),
                error_type: "timeout".into(),
                request_id: "req-1".into(),
                retry_count: 1,
            });
        }
        assert_eq!(engine.state.consecutive_failures.load(Ordering::SeqCst), 3);

        // One success resets
        engine.on_inference_completed(&InferenceCompletedPayload {
            model: "llama3".into(),
            model_version: "v1.0".into(),
            latency_ms: 100,
            tokens_generated: 50,
            request_id: "req-1".into(),
        });
        assert_eq!(engine.state.consecutive_failures.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_rollback_after_threshold() {
        let mut config = ReasoningConfig::default();
        config.max_consecutive_failures = 2;
        let engine = ReasoningEngine::new(config);

        // 1st failure — no action
        let result = engine.on_inference_failed(&InferenceFailedPayload {
            model: "llama3".into(),
            model_version: "v1.0".into(),
            error: "oom".into(),
            error_type: "oom".into(),
            request_id: "req-1".into(),
            retry_count: 1,
        });
        assert!(result.is_none());

        // 2nd failure — triggers rollback
        let result = engine.on_inference_failed(&InferenceFailedPayload {
            model: "llama3".into(),
            model_version: "v1.0".into(),
            error: "oom".into(),
            error_type: "oom".into(),
            request_id: "req-2".into(),
            retry_count: 2,
        });
        assert!(result.is_some());
        if let AiPayload::Rollback(rb) = &result.unwrap().payload {
            assert_eq!(rb.failure_count, 2);
            assert_eq!(rb.from_version, "v1.0");
        } else {
            panic!("Expected rollback action");
        }
    }

    #[test]
    fn test_queue_scale_up() {
        let config = ReasoningConfig::default();
        let engine = ReasoningEngine::new(config);
        engine.state.current_replicas.store(2, Ordering::SeqCst);

        let result = engine.on_queue_depth(&QueueDepthPayload {
            queue_name: "llama-inference".into(),
            depth: 90,
            capacity: 100,
            avg_wait_ms: 5000.0,
        });

        assert!(result.is_some());
        if let AiPayload::Scale(scale) = &result.unwrap().payload {
            assert!(matches!(scale.direction, ScaleDirection::Up));
        } else {
            panic!("Expected scale action");
        }
    }

    #[test]
    fn test_critical_risk_triggers_alert() {
        let config = ReasoningConfig::default();
        let engine = ReasoningEngine::new(config);

        let result = engine.on_pipeline_output(&PipelineOutputPayload {
            session_id: "sess-1".into(),
            decision: "model_degradation".into(),
            risk_level: RiskLevel::Critical,
            model: "llama3".into(),
            confidence: 0.99,
            reasoning: "Inference failures exceed threshold".into(),
        });

        assert!(result.is_some());
    }
}
