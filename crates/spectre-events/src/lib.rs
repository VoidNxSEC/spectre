//! # SPECTRE Events
//!
//! Event-driven messaging abstraction for the SPECTRE Fleet.
//!
//! This crate provides:
//! - **NATS client wrapper** with connection management
//! - **Event schema** definitions and validation
//! - **Publisher/Subscriber** traits and implementations
//! - **Request/Reply** patterns for RPC-style calls
//!
//! ## Architecture
//!
//! All SPECTRE services communicate via events published to NATS:
//! - **Commands**: Request-response (e.g., llm.request.v1 → llm.response.v1)
//! - **Events**: Fire-and-forget (e.g., system.metrics.v1, cost.incurred.v1)
//! - **Governance**: DAO/ZK proofs (e.g., governance.proposal.v1)
//!
//! ## Example
//!
//! ```rust,no_run
//! use spectre_events::{EventBus, Event, EventType};
//! use spectre_core::{ServiceId, CorrelationId};
//!
//! # async fn example() -> spectre_core::Result<()> {
//! // Connect to NATS
//! let bus = EventBus::connect("nats://localhost:4222").await?;
//!
//! // Create an event
//! let event = Event::new(
//!     EventType::SystemMetrics,
//!     ServiceId::new("agent-os"),
//!     serde_json::json!({"cpu": 45.2, "memory": 2048}),
//! );
//!
//! // Publish event
//! bus.publish(&event).await?;
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod event;
pub mod publisher;
pub mod subscriber;

// Re-exports
pub use client::{EventBus, EventBusConfig};
pub use event::{Event, EventMetadata, EventPayload, EventType};
pub use publisher::Publisher;
pub use subscriber::{EventHandler, Subscriber};

// Re-export spectre-core types for convenience
pub use spectre_core::{CorrelationId, Result, ServiceId, SpectreError, TraceId};
