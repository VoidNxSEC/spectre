//! NATS client wrapper for SPECTRE Fleet

use crate::event::Event;
use async_nats::{Client, ConnectOptions, ServerAddr};
use spectre_core::{Result, SpectreError};
use std::time::Duration;
use tracing::{debug, info, warn};

/// EventBus configuration
#[derive(Debug, Clone)]
pub struct EventBusConfig {
    /// NATS server URLs
    pub urls: Vec<String>,

    /// Max reconnect attempts
    pub max_reconnects: u32,

    /// Reconnect delay
    pub reconnect_delay: Duration,

    /// Connection name (for monitoring)
    pub name: String,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            urls: vec!["nats://localhost:4222".to_string()],
            max_reconnects: 10,
            reconnect_delay: Duration::from_secs(1),
            name: "spectre-event-bus".to_string(),
        }
    }
}

/// SPECTRE Event Bus
///
/// Wraps the NATS client with SPECTRE-specific functionality.
pub struct EventBus {
    client: Client,
    #[allow(dead_code)]
    config: EventBusConfig,
}

impl EventBus {
    /// Connect to NATS with default config
    pub async fn connect(url: impl Into<String>) -> Result<Self> {
        let config = EventBusConfig {
            urls: vec![url.into()],
            ..Default::default()
        };

        Self::connect_with_config(config).await
    }

    /// Connect to NATS with custom config
    pub async fn connect_with_config(config: EventBusConfig) -> Result<Self> {
        info!(
            urls = ?config.urls,
            name = %config.name,
            "Connecting to NATS"
        );

        // Parse server addresses
        let addrs: Vec<ServerAddr> = config
            .urls
            .iter()
            .filter_map(|url| url.parse().ok())
            .collect();

        if addrs.is_empty() {
            return Err(SpectreError::event_bus("No valid NATS URLs provided"));
        }

        // Build connection options with reconnection support
        let reconnect_delay = config.reconnect_delay;
        let options = ConnectOptions::new()
            .name(&config.name)
            .retry_on_initial_connect()
            .reconnect_delay_callback(move |attempts| {
                let delay = reconnect_delay * attempts as u32;
                delay
            })
            .event_callback(|event| async move {
                match event {
                    async_nats::Event::Connected => {
                        info!("NATS connected");
                    }
                    async_nats::Event::Disconnected => {
                        warn!("NATS disconnected");
                    }
                    other => {
                        warn!("NATS event: {}", other);
                    }
                }
            });

        // Connect
        let client = async_nats::connect_with_options(addrs, options)
            .await
            .map_err(|e| SpectreError::event_bus(format!("Failed to connect to NATS: {}", e)))?;

        info!("Connected to NATS successfully");

        Ok(Self { client, config })
    }

    /// Publish an event
    pub async fn publish(&self, event: &Event) -> Result<()> {
        let subject = event.subject();
        let payload = event.to_json()?;

        debug!(
            subject = %subject,
            event_id = %event.event_id,
            correlation_id = %event.correlation_id,
            "Publishing event"
        );

        self.client
            .publish(subject, payload.into())
            .await
            .map_err(|e| SpectreError::event_bus(format!("Failed to publish event: {}", e)))?;

        Ok(())
    }

    /// Publish an event and wait for response (request-reply pattern)
    pub async fn request(&self, event: &Event, timeout: Duration) -> Result<Event> {
        let subject = event.subject();
        let payload = event.to_json()?;

        debug!(
            subject = %subject,
            event_id = %event.event_id,
            ?timeout,
            "Sending request"
        );

        let response = tokio::time::timeout(
            timeout,
            self.client.request(subject.clone(), payload.into()),
        )
        .await
        .map_err(|_| SpectreError::timeout(format!("request to {}", subject), timeout.as_secs()))?
        .map_err(|e| SpectreError::event_bus(format!("Request failed: {}", e)))?;

        let response_str = String::from_utf8(response.payload.to_vec())
            .map_err(|e| SpectreError::event_bus(format!("Invalid UTF-8 in response: {}", e)))?;

        Event::from_json(&response_str)
    }

    /// Subscribe to a subject
    pub async fn subscribe(&self, subject: impl Into<String>) -> Result<async_nats::Subscriber> {
        let subject = subject.into();

        debug!(subject = %subject, "Subscribing to subject");

        self.client
            .subscribe(subject)
            .await
            .map_err(|e| SpectreError::event_bus(format!("Failed to subscribe: {}", e)))
    }

    /// Subscribe to a queue group (load-balanced)
    pub async fn queue_subscribe(
        &self,
        subject: impl Into<String>,
        queue: impl Into<String>,
    ) -> Result<async_nats::Subscriber> {
        let subject = subject.into();
        let queue = queue.into();

        debug!(
            subject = %subject,
            queue = %queue,
            "Subscribing to queue group"
        );

        self.client
            .queue_subscribe(subject, queue)
            .await
            .map_err(|e| SpectreError::event_bus(format!("Failed to queue subscribe: {}", e)))
    }

    /// Get the underlying NATS client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        matches!(
            self.client.connection_state(),
            async_nats::connection::State::Connected
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires NATS server running
    async fn test_connect() {
        let bus = EventBus::connect("nats://localhost:4222").await;
        assert!(bus.is_ok());
    }

    #[tokio::test]
    #[ignore] // Requires NATS server running
    async fn test_publish() {
        use crate::event::EventType;
        use spectre_core::ServiceId;

        let bus = EventBus::connect("nats://localhost:4222").await.unwrap();

        let event = Event::new(
            EventType::SystemMetrics,
            ServiceId::new("test"),
            serde_json::json!({"cpu": 50}),
        );

        let result = bus.publish(&event).await;
        assert!(result.is_ok());
    }
}
