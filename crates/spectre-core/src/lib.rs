//! # SPECTRE Core
//!
//! Core types, traits, and utilities for the SPECTRE Fleet framework.
//!
//! This crate provides foundational abstractions used across all SPECTRE services:
//! - **Identity types**: ServiceId, CorrelationId, TraceId
//! - **Error handling**: SpectreError with context
//! - **Configuration**: Unified config management
//! - **Logging**: Structured logging setup
//!
//! ## Architecture
//!
//! SPECTRE is an **Event-Driven Microservices** framework with:
//! - Zero-Trust governance (all traffic via Spectre Proxy)
//! - Hybrid Cloud + Local AI (Vertex AI + ml-offload-api)
//! - FinOps cost optimization
//! - Observability intelligence
//!
//! ## Example
//!
//! ```rust
//! use spectre_core::{ServiceId, CorrelationId, Result};
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let service_id = ServiceId::new("llm-gateway");
//!     let correlation_id = CorrelationId::generate();
//!
//!     tracing::info!(
//!         service = %service_id,
//!         correlation_id = %correlation_id,
//!         "Service initialized"
//!     );
//!
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod error;
pub mod logging;
pub mod shutdown;
pub mod types;

// Re-exports for convenience
pub use config::{Config, ConfigLoader};
pub use error::{Result, SpectreError};
pub use logging::init_logging;
pub use shutdown::shutdown_signal;
pub use types::{CorrelationId, ServiceId, TraceId};
