use crate::cloudflare::CloudflareClient;
use spectre_core::{Result, ServiceId, SpectreError};
use spectre_events::{Event, EventBus, EventHandler, EventType};
use std::sync::Arc;
use tracing::{error, info};

fn cf_err(e: impl std::fmt::Display) -> SpectreError {
    SpectreError::internal(format!("Cloudflare: {}", e))
}

pub struct DeploymentHandler {
    cf: Arc<CloudflareClient>,
    event_bus: Arc<EventBus>,
}

impl DeploymentHandler {
    pub fn new(cf: Arc<CloudflareClient>, event_bus: Arc<EventBus>) -> Self { Self { cf, event_bus } }

    async fn handle_requested(&self, event: Event) -> Result<()> {
        let project_id = event.payload["project_id"].as_str().unwrap_or("unknown");
        let environment = event.payload["environment"].as_str().unwrap_or("preview");
        let branch = event.payload["branch"].as_str().unwrap_or("main");

        info!(project_id = %project_id, environment = %environment, branch = %branch, "Deployment requested");

        // Verify Cloudflare API reachable
        match self.cf.list_workers().await {
            Ok(_) => info!("Cloudflare Workers API reachable"),
            Err(e) => error!(error = %e, "Workers API unreachable"),
        }

        let status_event = Event::new(EventType::DeploymentStatus, ServiceId::new("spectre-proxy"),
            serde_json::json!({"deployment_id": event.event_id.to_string(), "project_id": project_id, "status": "accepted", "environment": environment}));
        self.event_bus.publish(&status_event).await
    }
}

#[async_trait::async_trait]
impl EventHandler for DeploymentHandler {
    async fn handle(&self, event: Event) -> Result<()> {
        match &event.event_type {
            EventType::DeploymentRequested => self.handle_requested(event).await,
            other => { info!("Unexpected: {:?}", other); Ok(()) }
        }
    }
}
