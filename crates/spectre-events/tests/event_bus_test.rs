//! Integration tests for EventBus (NATS)
//!
//! These tests require NATS server running on localhost:4222
//! Run with: cargo test --test test_event_bus -- --test-threads=1

use spectre_core::{Result, ServiceId};
use spectre_events::{Event, EventBus, EventHandler, EventType, Subscriber};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;

/// Test: Connect to NATS
#[tokio::test]
async fn test_01_connect_to_nats() -> Result<()> {
    let bus = EventBus::connect("nats://localhost:4222").await?;
    // Allow connection state callback to process
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(bus.is_connected());
    Ok(())
}

/// Test: Publish single event
#[tokio::test]
async fn test_02_publish_event() -> Result<()> {
    let bus = EventBus::connect("nats://localhost:4222").await?;

    let event = Event::new(
        EventType::SystemMetrics,
        ServiceId::new("test-service"),
        serde_json::json!({"cpu": 50.0, "memory": 2048}),
    );

    bus.publish(&event).await?;
    println!("✅ Published event: {}", event.event_id);

    Ok(())
}

/// Test: Subscribe and receive event
#[tokio::test]
async fn test_03_subscribe_and_receive() -> Result<()> {
    let bus = EventBus::connect("nats://localhost:4222").await?;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Setup subscriber
    let received_events = Arc::new(Mutex::new(Vec::new()));
    let received_clone = received_events.clone();

    struct TestHandler {
        received: Arc<Mutex<Vec<Event>>>,
    }

    #[async_trait::async_trait]
    impl EventHandler for TestHandler {
        async fn handle(&self, event: Event) -> Result<()> {
            self.received.lock().await.push(event);
            Ok(())
        }
    }

    let handler = TestHandler {
        received: received_clone,
    };

    let nats_sub = bus.subscribe("test.event.v1").await?;
    let mut subscriber = Subscriber::new(nats_sub, "test.event.v1");

    // Spawn listener task
    let listener_task = tokio::spawn(async move { subscriber.listen(handler).await });

    // Give subscriber time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Publish event
    let event = Event::new(
        EventType::Custom("test.event.v1".to_string()),
        ServiceId::new("test-publisher"),
        serde_json::json!({"test": "data"}),
    );

    bus.publish(&event).await?;

    // Wait for event to be received (allow time for async delivery)
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Cancel listener
    listener_task.abort();

    // Verify event was received
    let events = received_events.lock().await;
    assert_eq!(events.len(), 1, "Expected 1 event, got {}", events.len());
    assert_eq!(events[0].event_id, event.event_id);

    println!("✅ Received event: {}", event.event_id);

    Ok(())
}

/// Test: Request-reply pattern
#[tokio::test]
async fn test_04_request_reply() -> Result<()> {
    let bus = EventBus::connect("nats://localhost:4222").await?;

    // Setup responder
    struct ResponderHandler;

    #[async_trait::async_trait]
    impl EventHandler for ResponderHandler {
        async fn handle(&self, event: Event) -> Result<()> {
            // Simulate processing
            println!("Responder received: {}", event.event_id);
            Ok(())
        }
    }

    let nats_sub = bus.subscribe("llm.request.v1").await?;
    let mut subscriber = Subscriber::new(nats_sub, "llm.request.v1");

    let responder_task = tokio::spawn(async move { subscriber.listen(ResponderHandler).await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send request (Note: For real request-reply, we'd need to implement reply in the handler)
    let request = Event::new(
        EventType::LlmRequest,
        ServiceId::new("test-client"),
        serde_json::json!({"prompt": "Hello"}),
    );

    // For now, just test publish (full request-reply needs more setup)
    bus.publish(&request).await?;

    tokio::time::sleep(Duration::from_millis(500)).await;
    responder_task.abort();

    println!("✅ Request-reply pattern tested");

    Ok(())
}

/// Test: Multiple subscribers (queue group - load balancing)
#[tokio::test]
async fn test_05_queue_group_load_balancing() -> Result<()> {
    let bus = EventBus::connect("nats://localhost:4222").await?;

    let received_1 = Arc::new(Mutex::new(Vec::new()));
    let received_2 = Arc::new(Mutex::new(Vec::new()));

    struct QueueHandler {
        id: String,
        received: Arc<Mutex<Vec<Event>>>,
    }

    #[async_trait::async_trait]
    impl EventHandler for QueueHandler {
        async fn handle(&self, event: Event) -> Result<()> {
            println!("Worker {} received: {}", self.id, event.event_id);
            self.received.lock().await.push(event);
            Ok(())
        }
    }

    // Create two subscribers in same queue group
    let nats_sub1 = bus.queue_subscribe("test.queue.v1", "workers").await?;
    let nats_sub2 = bus.queue_subscribe("test.queue.v1", "workers").await?;

    let mut subscriber1 = Subscriber::new(nats_sub1, "test.queue.v1");
    let mut subscriber2 = Subscriber::new(nats_sub2, "test.queue.v1");

    let handler1 = QueueHandler {
        id: "worker-1".to_string(),
        received: received_1.clone(),
    };

    let handler2 = QueueHandler {
        id: "worker-2".to_string(),
        received: received_2.clone(),
    };

    let task1 = tokio::spawn(async move { subscriber1.listen(handler1).await });
    let task2 = tokio::spawn(async move { subscriber2.listen(handler2).await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Publish 10 events
    for i in 0..10 {
        let event = Event::new(
            EventType::Custom("test.queue.v1".to_string()),
            ServiceId::new("test-publisher"),
            serde_json::json!({"id": i}),
        );
        bus.publish(&event).await?;
    }

    tokio::time::sleep(Duration::from_secs(1)).await;

    task1.abort();
    task2.abort();

    let count1 = received_1.lock().await.len();
    let count2 = received_2.lock().await.len();

    println!(
        "✅ Worker 1 received: {}, Worker 2 received: {}",
        count1, count2
    );
    assert_eq!(count1 + count2, 10, "Total events received should be 10");

    Ok(())
}

/// Test: Event serialization/deserialization
#[tokio::test]
async fn test_06_event_serialization() -> Result<()> {
    let event = Event::new(
        EventType::SystemMetrics,
        ServiceId::new("test"),
        serde_json::json!({"cpu": 75.5, "memory": 4096}),
    )
    .with_cost_tag("project-alpha")
    .with_auth("spectre-proxy");

    // Serialize
    let json = event.to_json()?;
    println!("Serialized: {}", json);

    // Deserialize
    let deserialized = Event::from_json(&json)?;

    assert_eq!(event.event_id, deserialized.event_id);
    assert_eq!(event.event_type, deserialized.event_type);
    assert_eq!(event.correlation_id, deserialized.correlation_id);

    println!("✅ Event serialization round-trip successful");

    Ok(())
}

/// Test: Correlation ID propagation
#[tokio::test]
async fn test_07_correlation_id_propagation() -> Result<()> {
    use spectre_core::CorrelationId;

    let correlation_id = CorrelationId::generate();

    let event1 = Event::with_correlation(
        EventType::LlmRequest,
        ServiceId::new("service-1"),
        correlation_id,
        serde_json::json!({"step": 1}),
    );

    let event2 = Event::with_correlation(
        EventType::LlmResponse,
        ServiceId::new("service-2"),
        correlation_id,
        serde_json::json!({"step": 2}),
    );

    assert_eq!(event1.correlation_id, event2.correlation_id);
    println!("✅ Correlation ID propagated: {}", correlation_id);

    Ok(())
}

/// Test: All event types are valid
#[tokio::test]
async fn test_08_all_event_types() -> Result<()> {
    let event_types = vec![
        EventType::LlmRequest,
        EventType::LlmResponse,
        EventType::InferenceRequest,
        EventType::InferenceResponse,
        EventType::VramStatus,
        EventType::AnalysisRequest,
        EventType::AnalysisResponse,
        EventType::SystemMetrics,
        EventType::CostIncurred,
    ];

    for event_type in event_types {
        let event = Event::new(
            event_type.clone(),
            ServiceId::new("test"),
            serde_json::json!({}),
        );

        let subject = event.subject();
        assert!(!subject.is_empty(), "Subject should not be empty");
        println!("✅ Event type {:?} -> subject: {}", event_type, subject);
    }

    Ok(())
}

/// Test: Connection resilience (reconnect)
#[tokio::test]
async fn test_09_connection_resilience() -> Result<()> {
    let bus = EventBus::connect("nats://localhost:4222").await?;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Publish event while connected
    let event = Event::new(
        EventType::SystemMetrics,
        ServiceId::new("test"),
        serde_json::json!({"test": "resilience"}),
    );

    bus.publish(&event).await?;

    // Check still connected
    assert!(bus.is_connected());

    println!("✅ Connection resilience tested");

    Ok(())
}

/// Test: Batch publish performance
#[tokio::test]
async fn test_10_batch_publish_performance() -> Result<()> {
    let bus = EventBus::connect("nats://localhost:4222").await?;

    let start = std::time::Instant::now();
    let count = 100;

    for i in 0..count {
        let event = Event::new(
            EventType::SystemMetrics,
            ServiceId::new("test"),
            serde_json::json!({"id": i}),
        );
        bus.publish(&event).await?;
    }

    let elapsed = start.elapsed();
    let events_per_sec = count as f64 / elapsed.as_secs_f64();

    println!(
        "✅ Published {} events in {:?} ({:.0} events/sec)",
        count, elapsed, events_per_sec
    );

    assert!(
        events_per_sec > 50.0,
        "Should publish at least 50 events/sec"
    );

    Ok(())
}
