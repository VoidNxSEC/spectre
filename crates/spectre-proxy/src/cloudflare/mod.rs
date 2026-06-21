//! Cloudflare API client for DNS + Workers management.
//!
//! Reads credentials from `CF_API_TOKEN` and `CF_ACCOUNT_ID` env vars.
//! In production these come from SOPS-encrypted secrets.

use anyhow::{Context, Result};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;

const CF_API_BASE: &str = "https://api.cloudflare.com/client/v4";

/// Top-level Cloudflare API client.
#[derive(Clone)]
pub struct CloudflareClient {
    http: HttpClient,
    token: String,
    pub account_id: String,
}

impl CloudflareClient {
    /// Build a client from environment variables.
    pub fn from_env() -> Result<Self> {
        let token = env::var("CF_API_TOKEN")
            .context("CF_API_TOKEN not set — check SOPS secrets/dev.enc.env")?;
        let account_id = env::var("CF_ACCOUNT_ID")
            .context("CF_ACCOUNT_ID not set — check SOPS secrets/dev.enc.env")?;

        let http = HttpClient::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()?;

        Ok(Self { http, token, account_id })
    }

    pub async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<CfResponse<T>> {
        let url = format!("{}{}", CF_API_BASE, path);
        let resp = self.http.get(&url).bearer_auth(&self.token).send().await?;
        let body: CfResponse<T> = resp.json().await?;
        body.into_result()
    }

    pub async fn post<T: for<'de> Deserialize<'de>, B: Serialize>(&self, path: &str, body: &B) -> Result<CfResponse<T>> {
        let url = format!("{}{}", CF_API_BASE, path);
        let resp = self.http.post(&url).bearer_auth(&self.token).json(body).send().await?;
        let resp_body: CfResponse<T> = resp.json().await?;
        resp_body.into_result()
    }

    pub async fn patch<T: for<'de> Deserialize<'de>, B: Serialize>(&self, path: &str, body: &B) -> Result<CfResponse<T>> {
        let url = format!("{}{}", CF_API_BASE, path);
        let resp = self.http.patch(&url).bearer_auth(&self.token).json(body).send().await?;
        let resp_body: CfResponse<T> = resp.json().await?;
        resp_body.into_result()
    }

    pub async fn delete<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<CfResponse<T>> {
        let url = format!("{}{}", CF_API_BASE, path);
        let resp = self.http.delete(&url).bearer_auth(&self.token).send().await?;
        let resp_body: CfResponse<T> = resp.json().await?;
        resp_body.into_result()
    }
}

// ── Response wrapper ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CfResponse<T> {
    pub success: bool,
    pub result: Option<T>,
    pub errors: Vec<CfError>,
    #[serde(default)]
    pub messages: Vec<String>,
}

impl<T> CfResponse<T> {
    fn into_result(self) -> Result<Self> {
        if self.success { Ok(self) } else {
            let msg = self.errors.iter().map(|e| &e.message).cloned().collect::<Vec<_>>().join("; ");
            anyhow::bail!("Cloudflare API error: {}", msg)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CfError {
    pub code: i64,
    pub message: String,
}
pub mod dns;
pub mod workers;
