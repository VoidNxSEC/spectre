//! Event handlers for Cloudflare domain + deployment operations.
//!
//! Each handler implements `spectre_events::EventHandler` and is spawned
//! as a long-running NATS subscriber task in `main.rs`.

pub mod domains;
pub mod subdomains;
pub mod deployments;
