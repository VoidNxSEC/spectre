//! Core identity types for SPECTRE Fleet
//!
//! These types provide unique identifiers for distributed tracing,
//! correlation, and service identification across the fleet.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Service identifier
///
/// Uniquely identifies a service in the SPECTRE fleet.
///
/// # Examples
///
/// ```
/// use spectre_core::ServiceId;
///
/// let service = ServiceId::new("llm-gateway");
/// assert_eq!(service.as_str(), "llm-gateway");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ServiceId(String);

impl ServiceId {
    /// Create a new ServiceId
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the service ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ServiceId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ServiceId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Correlation ID for request tracking
///
/// Links related events across service boundaries. All events
/// related to a single user request share the same correlation ID.
///
/// # Examples
///
/// ```
/// use spectre_core::CorrelationId;
///
/// let correlation_id = CorrelationId::generate();
/// println!("Request correlation: {}", correlation_id);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CorrelationId(Uuid);

impl CorrelationId {
    /// Generate a new random correlation ID
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create from an existing UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Get the inner UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Get as hyphenated string
    pub fn as_hyphenated(&self) -> String {
        self.0.hyphenated().to_string()
    }
}

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

impl From<Uuid> for CorrelationId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

/// Trace ID for distributed tracing (OpenTelemetry compatible)
///
/// Used for end-to-end tracing across all services. Compatible with
/// OpenTelemetry trace context propagation.
///
/// # Examples
///
/// ```
/// use spectre_core::TraceId;
///
/// let trace_id = TraceId::generate();
/// assert_eq!(trace_id.as_hex().len(), 32); // 16 bytes in hex
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TraceId([u8; 16]);

impl TraceId {
    /// Generate a new random trace ID
    pub fn generate() -> Self {
        Self(Uuid::new_v4().into_bytes())
    }

    /// Create from a 16-byte array
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Get the bytes
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Get as hex string (32 characters)
    pub fn as_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Parse from hex string
    pub fn from_hex(hex_str: &str) -> Result<Self, hex::FromHexError> {
        let mut bytes = [0u8; 16];
        hex::decode_to_slice(hex_str, &mut bytes)?;
        Ok(Self(bytes))
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_hex())
    }
}

impl Default for TraceId {
    fn default() -> Self {
        Self::generate()
    }
}

// Add hex dependency for TraceId hex encoding
// We'll need to add this to Cargo.toml

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_id() {
        let service = ServiceId::new("test-service");
        assert_eq!(service.as_str(), "test-service");
        assert_eq!(service.to_string(), "test-service");
    }

    #[test]
    fn test_correlation_id() {
        let id1 = CorrelationId::generate();
        let id2 = CorrelationId::generate();
        assert_ne!(id1, id2);

        let uuid = Uuid::new_v4();
        let id3 = CorrelationId::from_uuid(uuid);
        assert_eq!(id3.as_uuid(), &uuid);
    }

    #[test]
    fn test_trace_id() {
        let id1 = TraceId::generate();
        let id2 = TraceId::generate();
        assert_ne!(id1, id2);

        let hex = id1.as_hex();
        assert_eq!(hex.len(), 32);

        let id3 = TraceId::from_hex(&hex).unwrap();
        assert_eq!(id1, id3);
    }

    #[test]
    fn test_serialization() {
        let service = ServiceId::new("test");
        let json = serde_json::to_string(&service).unwrap();
        let deserialized: ServiceId = serde_json::from_str(&json).unwrap();
        assert_eq!(service, deserialized);

        let correlation = CorrelationId::generate();
        let json = serde_json::to_string(&correlation).unwrap();
        let deserialized: CorrelationId = serde_json::from_str(&json).unwrap();
        assert_eq!(correlation, deserialized);
    }
}
