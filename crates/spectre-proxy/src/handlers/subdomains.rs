use crate::cloudflare::CloudflareClient;
use spectre_core::{Result, ServiceId, SpectreError};
use spectre_events::{Event, EventBus, EventHandler, EventType};
use std::sync::Arc;
use tracing::{info, warn};

fn cf_err(e: impl std::fmt::Display) -> SpectreError {
    SpectreError::internal(format!("Cloudflare: {}", e))
}

pub struct SubdomainHandler {
    cf: Arc<CloudflareClient>,
    event_bus: Arc<EventBus>,
}

impl SubdomainHandler {
    pub fn new(cf: Arc<CloudflareClient>, event_bus: Arc<EventBus>) -> Self { Self { cf, event_bus } }

    async fn find_zone_id(&self, domain_name: &str) -> Result<Option<String>> {
        let zones = self.cf.list_zones().await.map_err(cf_err)?.result.unwrap_or_default();
        Ok(zones.into_iter().find(|z| z.name == domain_name).map(|z| z.id))
    }

    async fn handle_created(&self, event: Event) -> Result<()> {
        let _prefix = event.payload["prefix"].as_str().unwrap_or("");
        let full_name = event.payload["full_name"].as_str().unwrap_or("?");
        let target = event.payload["target"].as_str().unwrap_or("");
        let record_type = event.payload["record_type"].as_str().unwrap_or("A");
        let ttl = event.payload["ttl"].as_i64().unwrap_or(3600);
        let domain_name = full_name.splitn(2, '.').nth(1).unwrap_or(full_name);

        let zone_id = match self.find_zone_id(domain_name).await? {
            Some(id) => id,
            None => { warn!(domain = %domain_name, "Zone not found"); return Ok(()); }
        };

        info!(zone_id = %zone_id, record = %full_name, "Creating DNS record");
        self.cf.create_dns_record(&zone_id, record_type, full_name, target, ttl, false).await.map_err(cf_err)?;

        let synced = Event::new(EventType::CloudflareDnsSynced, ServiceId::new("spectre-proxy"),
            serde_json::json!({"subdomain_id": event.payload["id"], "zone_id": zone_id, "record_name": full_name}));
        self.event_bus.publish(&synced).await?;
        Ok(())
    }

    async fn handle_updated(&self, _event: Event) -> Result<()> {
        info!("DNS record update — needs zone/record ID mapping"); Ok(())
    }

    async fn handle_deleted(&self, _event: Event) -> Result<()> {
        info!("DNS record delete — needs zone/record ID mapping"); Ok(())
    }
}

#[async_trait::async_trait]
impl EventHandler for SubdomainHandler {
    async fn handle(&self, event: Event) -> Result<()> {
        match &event.event_type {
            EventType::SubdomainCreated => self.handle_created(event).await,
            EventType::SubdomainUpdated => self.handle_updated(event).await,
            EventType::SubdomainDeleted => self.handle_deleted(event).await,
            other => { warn!("Unexpected: {:?}", other); Ok(()) }
        }
    }
}
