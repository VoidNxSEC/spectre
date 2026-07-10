// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 VoidNXLabs
//
// SPECTRE AI Reactor — event-driven AI backbone.
//
// Consumes AI events from NATS, applies reasoning rules,
// and emits reactive actions (scale, rollback, alert).
//
// Architecture:
//   ml-ops-api ──► ml_offload.inference.* ──► Reactor ──► spectre.ai.*
//   neoland     ──► neoland.pipeline.*    ──►          │
//   sentinel    ──► sentinel.alert.*      ──►          │
//                                                       ▼
//                                              NATS → KEDA / Grafana / ml-ops-api

pub mod events;
pub mod reasoning;

use reasoning::ReasoningConfig;
use std::sync::Arc;
use tokio_stream::StreamExt;
use tracing::{error, info};

/// Main AI Reactor service.
pub struct AiReactor {
    engine: ReasoningEngine,
    nats_url: String,
}

impl AiReactor {
    pub async fn new(nats_url: String, config: ReasoningConfig) -> anyhow::Result<Self> {
        info!("SPECTRE AI Reactor starting — NATS: {}", nats_url);
        Ok(Self {
            engine: ReasoningEngine::new(config),
            nats_url,
        })
    }

    /// Start consuming AI events and reacting.
    pub async fn run(self: Arc<Self>) -> anyhow::Result<()> {
        let nc = async_nats::connect(&self.nats_url).await?;
        info!("Connected to NATS at {}", self.nats_url);

        // Consumer 1: ml_offload.inference.completed
        {
            let nc = nc.clone();
            let reactor = Arc::clone(&self);
            let mut sub = nc
                .subscribe(events::producers::INFERENCE_COMPLETED.to_string())
                .await?;

            tokio::spawn(async move {
                while let Some(msg) = sub.next().await {
                    match serde_json::from_slice::<events::AiEvent>(&msg.payload) {
                        Ok(event) => {
                            if let events::AiPayload::InferenceCompleted(payload) = &event.payload {
                                reactor.engine.on_inference_completed(payload);
                            }
                        }
                        Err(e) => {
                            error!("Failed to parse inference.completed: {}", e);
                        }
                    }
                }
            });
        }

        // Consumer 2: ml_offload.inference.failed
        {
            let nc = nc.clone();
            let reactor = Arc::clone(&self);
            let mut sub = nc
                .subscribe(events::producers::INFERENCE_FAILED.to_string())
                .await?;

            tokio::spawn(async move {
                while let Some(msg) = sub.next().await {
                    match serde_json::from_slice::<events::AiEvent>(&msg.payload) {
                        Ok(event) => {
                            if let events::AiPayload::InferenceFailed(payload) = &event.payload {
                                if let Some(action) = reactor.engine.on_inference_failed(payload) {
                                    let data = serde_json::to_vec(&action).unwrap_or_default();
                                    let _ = nc
                                        .publish(
                                            events::actions::AI_ROLLBACK.to_string(),
                                            data.into(),
                                        )
                                        .await;
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to parse inference.failed: {}", e);
                        }
                    }
                }
            });
        }

        // Consumer 3: neoland.pipeline.output.v1
        {
            let nc = nc.clone();
            let reactor = Arc::clone(&self);
            let mut sub = nc
                .subscribe(events::producers::PIPELINE_OUTPUT.to_string())
                .await?;

            tokio::spawn(async move {
                while let Some(msg) = sub.next().await {
                    match serde_json::from_slice::<events::AiEvent>(&msg.payload) {
                        Ok(event) => {
                            if let events::AiPayload::PipelineOutput(payload) = &event.payload {
                                if let Some(action) = reactor.engine.on_pipeline_output(payload) {
                                    let data = serde_json::to_vec(&action).unwrap_or_default();
                                    let _ = nc
                                        .publish(events::actions::AI_ALERT.to_string(), data.into())
                                        .await;
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to parse pipeline.output: {}", e);
                        }
                    }
                }
            });
        }

        // Consumer 4: sentinel.alert.v1
        {
            let nc = nc.clone();
            let reactor = Arc::clone(&self);
            let mut sub = nc
                .subscribe(events::producers::SENTINEL_ALERT.to_string())
                .await?;

            tokio::spawn(async move {
                while let Some(msg) = sub.next().await {
                    match serde_json::from_slice::<events::AiEvent>(&msg.payload) {
                        Ok(event) => {
                            if let events::AiPayload::SentinelAlert(payload) = &event.payload {
                                if let Some(action) = reactor.engine.on_sentinel_alert(payload) {
                                    let data = serde_json::to_vec(&action).unwrap_or_default();
                                    let _ = nc
                                        .publish(events::actions::AI_ALERT.to_string(), data.into())
                                        .await;
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to parse sentinel.alert: {}", e);
                        }
                    }
                }
            });
        }

        info!("AI Reactor running — consuming 4 event streams");
        tokio::signal::ctrl_c().await?;
        info!("AI Reactor shutting down");
        Ok(())
    }
}

// Re-export
pub use events::AiEvent;
pub use reasoning::ReasoningEngine;
