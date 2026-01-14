//! Event subscriber trait and implementations

use crate::event::Event;
use async_nats::Subscriber as NatsSubscriber;
use futures::StreamExt;
use spectre_core::Result;
use tracing::{debug, error, info};

/// Event handler trait
///
/// Implement this trait to handle incoming events.
#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an event
    ///
    /// Return Ok(()) to acknowledge the event, or Err to reject it.
    async fn handle(&self, event: Event) -> Result<()>;
}

/// Event subscriber
///
/// Listens for events on a NATS subject and dispatches them to a handler.
pub struct Subscriber {
    nats_subscriber: NatsSubscriber,
    subject: String,
}

impl Subscriber {
    /// Create a new subscriber
    pub fn new(nats_subscriber: NatsSubscriber, subject: impl Into<String>) -> Self {
        Self {
            nats_subscriber,
            subject: subject.into(),
        }
    }

    /// Start listening for events
    ///
    /// This is a long-running task that should be spawned in a separate tokio task.
    pub async fn listen<H>(&mut self, handler: H) -> Result<()>
    where
        H: EventHandler,
    {
        info!(subject = %self.subject, "Subscriber started");

        while let Some(message) = self.nats_subscriber.next().await {
            debug!(
                subject = %self.subject,
                size = message.payload.len(),
                "Received message"
            );

            // Parse event
            let event_str = match String::from_utf8(message.payload.to_vec()) {
                Ok(s) => s,
                Err(e) => {
                    error!(error = %e, "Invalid UTF-8 in message");
                    continue;
                }
            };

            let event = match Event::from_json(&event_str) {
                Ok(e) => e,
                Err(e) => {
                    error!(error = %e, "Failed to parse event");
                    continue;
                }
            };

            // Handle event
            if let Err(e) = handler.handle(event).await {
                error!(error = %e, "Handler failed");
                // Could implement retry logic here
            }
        }

        info!(subject = %self.subject, "Subscriber stopped");
        Ok(())
    }

    /// Unsubscribe
    pub async fn unsubscribe(mut self) -> Result<()> {
        self.nats_subscriber
            .unsubscribe()
            .await
            .map_err(|e| spectre_core::SpectreError::event_bus(format!("Unsubscribe failed: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventType;
    use spectre_core::ServiceId;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // Mock event handler for testing
    struct MockHandler {
        handled: Arc<Mutex<Vec<Event>>>,
    }

    #[async_trait::async_trait]
    impl EventHandler for MockHandler {
        async fn handle(&self, event: Event) -> Result<()> {
            self.handled.lock().await.push(event);
            Ok(())
        }
    }

    #[test]
    fn test_subscriber_creation() {
        // This test just verifies the structure compiles
        // Actual subscription testing requires NATS server
    }

    #[tokio::test]
    async fn test_event_handler() {
        let handled = Arc::new(Mutex::new(Vec::new()));
        let handler = MockHandler {
            handled: handled.clone(),
        };

        let event = Event::new(
            EventType::SystemMetrics,
            ServiceId::new("test"),
            serde_json::json!({}),
        );

        handler.handle(event.clone()).await.unwrap();

        let handled_events = handled.lock().await;
        assert_eq!(handled_events.len(), 1);
        assert_eq!(handled_events[0].event_id, event.event_id);
    }
}
