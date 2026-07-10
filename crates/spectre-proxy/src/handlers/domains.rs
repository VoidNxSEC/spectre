use crate::cloudflare::CloudflareClient;
use spectre_core::{Result, ServiceId, SpectreError};
use spectre_events::{Event, EventBus, EventHandler, EventType};
use std::sync::Arc;
use tracing::{info, warn};

fn cf_err(e: impl std::fmt::Display) -> SpectreError {
    SpectreError::internal(format!("Cloudflare: {}", e))
}

pub struct DomainHandler {
    cf: Arc<CloudflareClient>,
    event_bus: Arc<EventBus>,
}

impl DomainHandler {
    pub fn new(cf: Arc<CloudflareClient>, event_bus: Arc<EventBus>) -> Self { Self { cf, event_bus } }

    async fn handle_created(&self, event: Event) -> Result<()> {
        let name = event.payload["name"].as_str().unwrap_or("unknown");
        info!(domain = %name, "Creating Cloudflare zone");
        let resp = self.cf.create_zone(name).await.map_err(cf_err)?;
        let zone = resp.result.ok_or_else(|| SpectreError::internal("No zone in response"))?;
        info!(zone_id = %zone.id, "Zone created");
        let synced = Event::new(EventType::CloudflareDnsSynced, ServiceId::new("spectre-proxy"),
            serde_json::json!({"domain_id": event.payload["id"], "zone_id": zone.id, "name": zone.name}));
        self.event_bus.publish(&synced).await?;
        Ok(())
    }

    async fn handle_deleted(&self, event: Event) -> Result<()> {
        let name = event.payload["name"].as_str().unwrap_or("unknown");
        let zones = self.cf.list_zones().await.map_err(cf_err)?.result.unwrap_or_default();
        match zones.into_iter().find(|z| z.name == name) {
            Some(z) => { self.cf.delete_zone(&z.id).await.map_err(cf_err)?; info!(zone_id = %z.id, "Deleted"); }
            None => warn!(domain = %name, "Not found"),
        }
        Ok(())
    }

    async fn handle_updated(&self, event: Event) -> Result<()> {
        info!(domain = %event.payload["name"].as_str().unwrap_or("?"), "Update — no-op");
        Ok(())
    }

    async fn handle_list_request(&self, _event: Event) -> Result<()> {
        let zones = self.cf.list_zones().await.map_err(cf_err)?.result.unwrap_or_default();
        let reply = Event::new(EventType::DomainListResponse, ServiceId::new("spectre-proxy"),
            serde_json::json!({"domains": zones.iter().map(|z| serde_json::json!({"id":z.id,"name":z.name,"status":z.status})).collect::<Vec<_>>()}));
        self.event_bus.publish(&reply).await
    }
}

#[async_trait::async_trait]
impl EventHandler for DomainHandler {
    async fn handle(&self, event: Event) -> Result<()> {
        match &event.event_type {
            EventType::DomainCreated => self.handle_created(event).await,
            EventType::DomainUpdated => self.handle_updated(event).await,
            EventType::DomainDeleted => self.handle_deleted(event).await,
            EventType::DomainListRequest => self.handle_list_request(event).await,
            other => { warn!("Unexpected: {:?}", other); Ok(()) }
        }
    }
}
