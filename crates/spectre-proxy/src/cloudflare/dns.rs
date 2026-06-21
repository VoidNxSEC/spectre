//! Cloudflare DNS zone + record operations.

use super::{CfResponse, CloudflareClient};
use anyhow::Result;
use serde::{Deserialize, Serialize};

// ── Zone types ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Zone {
    pub id: String,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct CreateZoneRequest {
    pub name: String,
    pub account: AccountRef,
    #[serde(rename = "type")]
    pub zone_type: String,
}

#[derive(Debug, Serialize)]
pub struct AccountRef {
    pub id: String,
}

// ── DNS Record types ──────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DnsRecord {
    pub id: String,
    pub zone_id: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub name: String,
    pub content: String,
    pub ttl: i64,
    pub proxied: bool,
}

#[derive(Debug, Serialize)]
pub struct CreateDnsRecordRequest {
    #[serde(rename = "type")]
    pub record_type: String,
    pub name: String,
    pub content: String,
    pub ttl: i64,
    pub proxied: bool,
}

#[derive(Debug, Serialize)]
pub struct UpdateDnsRecordRequest {
    #[serde(rename = "type")]
    pub record_type: Option<String>,
    pub name: Option<String>,
    pub content: Option<String>,
    pub ttl: Option<i64>,
    pub proxied: Option<bool>,
}

impl CloudflareClient {
    pub async fn list_zones(&self) -> Result<CfResponse<Vec<Zone>>> {
        self.get(&format!("/zones?account.id={}", self.account_id)).await
    }

    pub async fn create_zone(&self, name: &str) -> Result<CfResponse<Zone>> {
        self.post("/zones", &CreateZoneRequest {
            name: name.to_string(),
            account: AccountRef { id: self.account_id.clone() },
            zone_type: "full".to_string(),
        }).await
    }

    pub async fn delete_zone(&self, zone_id: &str) -> Result<CfResponse<Zone>> {
        self.delete(&format!("/zones/{}", zone_id)).await
    }

    pub async fn create_dns_record(
        &self, zone_id: &str, record_type: &str, name: &str, content: &str, ttl: i64, proxied: bool,
    ) -> Result<CfResponse<DnsRecord>> {
        self.post(&format!("/zones/{}/dns_records", zone_id), &CreateDnsRecordRequest {
            record_type: record_type.to_string(), name: name.to_string(),
            content: content.to_string(), ttl, proxied,
        }).await
    }

    pub async fn update_dns_record(
        &self, zone_id: &str, record_id: &str, req: UpdateDnsRecordRequest,
    ) -> Result<CfResponse<DnsRecord>> {
        self.patch(&format!("/zones/{}/dns_records/{}", zone_id, record_id), &req).await
    }

    pub async fn delete_dns_record(&self, zone_id: &str, record_id: &str) -> Result<CfResponse<DnsRecord>> {
        self.delete(&format!("/zones/{}/dns_records/{}", zone_id, record_id)).await
    }
}
