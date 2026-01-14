//! Event publisher trait and implementations

use crate::event::Event;
use spectre_core::Result;

/// Event Publisher trait
///
/// Abstraction for publishing events to the event bus.
#[async_trait::async_trait]
pub trait Publisher: Send + Sync {
    /// Publish an event
    async fn publish(&self, event: &Event) -> Result<()>;

    /// Publish multiple events in a batch
    async fn publish_batch(&self, events: &[Event]) -> Result<()> {
        for event in events {
            self.publish(event).await?;
        }
        Ok(())
    }
}

// Implement Publisher for EventBus
#[async_trait::async_trait]
impl Publisher for crate::client::EventBus {
    async fn publish(&self, event: &Event) -> Result<()> {
        self.publish(event).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventType;
    use spectre_core::ServiceId;

    // Mock publisher for testing
    struct MockPublisher {
        published: std::sync::Arc<tokio::sync::Mutex<Vec<Event>>>,
    }

    #[async_trait::async_trait]
    impl Publisher for MockPublisher {
        async fn publish(&self, event: &Event) -> Result<()> {
            self.published.lock().await.push(event.clone());
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_mock_publisher() {
        let published = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let publisher = MockPublisher {
            published: published.clone(),
        };

        let event = Event::new(
            EventType::SystemMetrics,
            ServiceId::new("test"),
            serde_json::json!({}),
        );

        publisher.publish(&event).await.unwrap();

        let published_events = published.lock().await;
        assert_eq!(published_events.len(), 1);
        assert_eq!(published_events[0].event_id, event.event_id);
    }

    #[tokio::test]
    async fn test_publish_batch() {
        let published = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let publisher = MockPublisher {
            published: published.clone(),
        };

        let events: Vec<Event> = (0..5)
            .map(|i| {
                Event::new(
                    EventType::SystemMetrics,
                    ServiceId::new("test"),
                    serde_json::json!({"id": i}),
                )
            })
            .collect();

        publisher.publish_batch(&events).await.unwrap();

        let published_events = published.lock().await;
        assert_eq!(published_events.len(), 5);
    }
}
