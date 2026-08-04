//! Event handler for Cloudflare ops requests (voidnx-api → Spectre, request-reply).
//!
//! Unlike the domain/subdomain/deployment handlers, this does NOT use
//! `spectre_events::Subscriber` — that wrapper parses the NATS message into an
//! `Event` and discards `message.reply`, so a handler built on it can only ever
//! broadcast a response on a fixed subject, never answer the caller's inbox.
//! `voidnx-api` needs a real request-reply round trip (via `EventBus::request`,
//! which sets `message.reply` under the hood), so this handler subscribes to
//! the raw NATS subject directly and replies to `message.reply` explicitly.
//! Existing handlers are untouched.

use crate::cloudflare::CloudflareClient;
use futures::StreamExt;
use spectre_core::{Result, ServiceId, SpectreError};
use spectre_events::{Event, EventBus, EventType};
use std::sync::Arc;
use tracing::{error, info, warn};

fn cf_err(e: impl std::fmt::Display) -> SpectreError {
    SpectreError::internal(format!("Cloudflare: {}", e))
}

pub struct CloudflareHandler {
    cf: Arc<CloudflareClient>,
    event_bus: Arc<EventBus>,
}

impl CloudflareHandler {
    pub fn new(cf: Arc<CloudflareClient>, event_bus: Arc<EventBus>) -> Self {
        Self { cf, event_bus }
    }

    async fn handle_workers_list(&self) -> Result<serde_json::Value> {
        let workers = self
            .cf
            .list_workers()
            .await
            .map_err(cf_err)?
            .result
            .unwrap_or_default();

        Ok(serde_json::json!({
            "workers": workers.iter().map(|w| serde_json::json!({
                "id": w.id,
                "script_name": w.script_name,
                "created_on": w.created_on,
            })).collect::<Vec<_>>(),
            "count": workers.len(),
        }))
    }

    /// Long-running task: subscribe to `cloudflare.*.v1` and reply to each
    /// request's inbox subject, not a fixed broadcast subject.
    pub async fn listen(&self, subject: &str) -> Result<()> {
        let mut sub = self
            .event_bus
            .client()
            .subscribe(subject.to_string())
            .await
            .map_err(|e| {
                SpectreError::event_bus(format!("Failed to subscribe to {}: {}", subject, e))
            })?;

        info!(subject = %subject, "CloudflareHandler subscriber started");

        while let Some(message) = sub.next().await {
            let Some(reply_subject) = message.reply.clone() else {
                warn!(subject = %message.subject, "Cloudflare request with no reply subject — ignoring");
                continue;
            };

            let event = match std::str::from_utf8(&message.payload)
                .ok()
                .and_then(|s| Event::from_json(s).ok())
            {
                Some(e) => e,
                None => {
                    error!(subject = %message.subject, "Failed to parse Cloudflare request event");
                    continue;
                }
            };

            let result = match &event.event_type {
                EventType::CloudflareWorkersListRequest => self.handle_workers_list().await,
                other => {
                    warn!("Unexpected Cloudflare event type: {:?}", other);
                    continue;
                }
            };

            let response_type = event
                .event_type
                .response_type()
                .unwrap_or(EventType::CloudflareWorkersListResponse);

            let payload = match result {
                Ok(payload) => payload,
                Err(e) => {
                    error!(error = %e, "Cloudflare handler failed");
                    serde_json::json!({ "error": e.to_string() })
                }
            };

            let reply_event = Event::with_correlation(
                response_type,
                ServiceId::new("spectre-proxy"),
                event.correlation_id,
                payload,
            );

            let reply_json = match reply_event.to_json() {
                Ok(j) => j,
                Err(e) => {
                    error!(error = %e, "Failed to serialize Cloudflare reply");
                    continue;
                }
            };

            if let Err(e) = self
                .event_bus
                .client()
                .publish(reply_subject, reply_json.into())
                .await
            {
                error!(error = %e, "Failed to publish Cloudflare reply to inbox");
            }
        }

        info!(subject = %subject, "CloudflareHandler subscriber stopped");
        Ok(())
    }
}
