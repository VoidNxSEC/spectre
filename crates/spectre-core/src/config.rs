//! Configuration management for SPECTRE Fleet
//!
//! Provides unified configuration loading and validation.

use crate::error::{Result, SpectreError};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Main SPECTRE configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Service configuration
    pub service: ServiceConfig,

    /// NATS configuration
    pub nats: NatsConfig,

    /// Observability configuration
    #[serde(default)]
    pub observability: ObservabilityConfig,

    /// Security configuration
    #[serde(default)]
    pub security: SecurityConfig,
}

/// Service-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Service ID (e.g., "llm-gateway", "rag-service")
    pub id: String,

    /// Service name (human-readable)
    pub name: String,

    /// Service version
    #[serde(default = "default_version")]
    pub version: String,

    /// Environment (dev, staging, prod)
    #[serde(default = "default_environment")]
    pub environment: String,
}

/// NATS message bus configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatsConfig {
    /// NATS server URL (e.g., "nats://localhost:4222")
    #[serde(default = "default_nats_url")]
    pub url: String,

    /// JetStream enabled
    #[serde(default = "default_true")]
    pub jetstream: bool,

    /// Max reconnect attempts
    #[serde(default = "default_max_reconnects")]
    pub max_reconnects: u32,

    /// Reconnect delay in milliseconds
    #[serde(default = "default_reconnect_delay_ms")]
    pub reconnect_delay_ms: u64,
}

/// Observability configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ObservabilityConfig {
    /// Enable metrics export
    #[serde(default = "default_true")]
    pub metrics_enabled: bool,

    /// Enable tracing export
    #[serde(default = "default_true")]
    pub tracing_enabled: bool,

    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// TimescaleDB URL for observability storage
    pub timescaledb_url: Option<String>,

    /// Neo4j URI for dependency graph
    pub neo4j_uri: Option<String>,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecurityConfig {
    /// Enable TLS
    #[serde(default)]
    pub tls_enabled: bool,

    /// TLS certificate path
    pub tls_cert_path: Option<String>,

    /// TLS key path
    pub tls_key_path: Option<String>,

    /// Secrets service URL
    pub secrets_service_url: Option<String>,
}

// Default value functions
fn default_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn default_environment() -> String {
    std::env::var("SPECTRE_ENV").unwrap_or_else(|_| "dev".to_string())
}

fn default_nats_url() -> String {
    std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string())
}

fn default_true() -> bool {
    true
}

fn default_max_reconnects() -> u32 {
    10
}

fn default_reconnect_delay_ms() -> u64 {
    1000
}

fn default_log_level() -> String {
    std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
}

impl Config {
    /// Load configuration from TOML file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| SpectreError::config(format!("Failed to read config file: {}", e)))?;

        toml::from_str(&content)
            .map_err(|e| SpectreError::config(format!("Failed to parse config file: {}", e)))
    }

    /// Load configuration from environment variables (fallback)
    pub fn from_env() -> Result<Self> {
        let service_id = std::env::var("SPECTRE_SERVICE_ID")
            .map_err(|_| SpectreError::config("SPECTRE_SERVICE_ID not set"))?;

        let service_name =
            std::env::var("SPECTRE_SERVICE_NAME").unwrap_or_else(|_| service_id.clone());

        Ok(Self {
            service: ServiceConfig {
                id: service_id,
                name: service_name,
                version: default_version(),
                environment: default_environment(),
            },
            nats: NatsConfig {
                url: default_nats_url(),
                jetstream: true,
                max_reconnects: default_max_reconnects(),
                reconnect_delay_ms: default_reconnect_delay_ms(),
            },
            observability: ObservabilityConfig {
                metrics_enabled: true,
                tracing_enabled: true,
                log_level: default_log_level(),
                timescaledb_url: std::env::var("TIMESCALEDB_URL").ok(),
                neo4j_uri: std::env::var("NEO4J_URI").ok(),
            },
            security: SecurityConfig::default(),
        })
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.service.id.is_empty() {
            return Err(SpectreError::config("service.id cannot be empty"));
        }

        if self.nats.url.is_empty() {
            return Err(SpectreError::config("nats.url cannot be empty"));
        }

        Ok(())
    }
}

/// Configuration loader utility
pub struct ConfigLoader;

impl ConfigLoader {
    /// Load configuration with fallback strategy:
    /// 1. Try from file (if path provided)
    /// 2. Try from environment variables
    /// 3. Use defaults
    pub fn load(config_path: Option<impl AsRef<Path>>) -> Result<Config> {
        let config = if let Some(path) = config_path {
            Config::from_file(path)?
        } else {
            Config::from_env()?
        };

        config.validate()?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        std::env::set_var("SPECTRE_SERVICE_ID", "test-service");
        let mut config = Config::from_env().unwrap();
        assert!(config.validate().is_ok());

        config.service.id = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_default_values() {
        let config = NatsConfig {
            url: default_nats_url(),
            jetstream: default_true(),
            max_reconnects: default_max_reconnects(),
            reconnect_delay_ms: default_reconnect_delay_ms(),
        };

        assert_eq!(config.max_reconnects, 10);
        assert_eq!(config.reconnect_delay_ms, 1000);
        assert!(config.jetstream);
    }
}
