//! Error types for SPECTRE Fleet
//!
//! Provides a unified error type with context for all SPECTRE services.

use thiserror::Error;

/// SPECTRE Result type
pub type Result<T> = std::result::Result<T, SpectreError>;

/// SPECTRE Error with context
///
/// All SPECTRE services use this error type for consistency
/// and better error propagation across service boundaries.
#[derive(Error, Debug)]
pub enum SpectreError {
    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// NATS/Event bus error
    #[error("Event bus error: {0}")]
    EventBus(String),

    /// Database error
    #[error("Database error: {0}")]
    Database(String),

    /// Authentication/Authorization error
    #[error("Auth error: {0}")]
    Auth(String),

    /// Service unavailable
    #[error("Service '{service}' unavailable: {reason}")]
    ServiceUnavailable { service: String, reason: String },

    /// Rate limit exceeded
    #[error("Rate limit exceeded for '{resource}': {details}")]
    RateLimitExceeded { resource: String, details: String },

    /// Invalid request
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Not found
    #[error("{resource} not found: {id}")]
    NotFound { resource: String, id: String },

    /// Timeout
    #[error("Operation timed out after {seconds}s: {operation}")]
    Timeout { operation: String, seconds: u64 },

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Generic error (fallback)
    #[error("Internal error: {0}")]
    Internal(String),

    /// Wrapped error from anyhow (for external libraries)
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl SpectreError {
    /// Create a Config error
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create an EventBus error
    pub fn event_bus(msg: impl Into<String>) -> Self {
        Self::EventBus(msg.into())
    }

    /// Create a Database error
    pub fn database(msg: impl Into<String>) -> Self {
        Self::Database(msg.into())
    }

    /// Create an Auth error
    pub fn auth(msg: impl Into<String>) -> Self {
        Self::Auth(msg.into())
    }

    /// Create a ServiceUnavailable error
    pub fn service_unavailable(service: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ServiceUnavailable {
            service: service.into(),
            reason: reason.into(),
        }
    }

    /// Create a RateLimitExceeded error
    pub fn rate_limit(resource: impl Into<String>, details: impl Into<String>) -> Self {
        Self::RateLimitExceeded {
            resource: resource.into(),
            details: details.into(),
        }
    }

    /// Create an InvalidRequest error
    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self::InvalidRequest(msg.into())
    }

    /// Create a NotFound error
    pub fn not_found(resource: impl Into<String>, id: impl Into<String>) -> Self {
        Self::NotFound {
            resource: resource.into(),
            id: id.into(),
        }
    }

    /// Create a Timeout error
    pub fn timeout(operation: impl Into<String>, seconds: u64) -> Self {
        Self::Timeout {
            operation: operation.into(),
            seconds,
        }
    }

    /// Create an Internal error
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    /// Create a Serialization error (via Internal)
    pub fn serialization(msg: impl Into<String>) -> Self {
        Self::Internal(format!("Serialization error: {}", msg.into()))
    }

    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::ServiceUnavailable { .. } | Self::Timeout { .. } | Self::EventBus(_)
        )
    }

    /// Check if error is a client error (4xx equivalent)
    pub fn is_client_error(&self) -> bool {
        matches!(
            self,
            Self::InvalidRequest(_) | Self::NotFound { .. } | Self::Auth(_)
        )
    }

    /// Get HTTP status code equivalent
    pub fn status_code(&self) -> u16 {
        match self {
            Self::InvalidRequest(_) => 400,
            Self::Auth(_) => 401,
            Self::NotFound { .. } => 404,
            Self::RateLimitExceeded { .. } => 429,
            Self::ServiceUnavailable { .. } => 503,
            Self::Timeout { .. } => 504,
            _ => 500,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_construction() {
        let err = SpectreError::config("invalid config");
        assert!(matches!(err, SpectreError::Config(_)));

        let err = SpectreError::service_unavailable("llm-gateway", "connection refused");
        assert!(err.is_retryable());
        assert_eq!(err.status_code(), 503);

        let err = SpectreError::not_found("model", "gpt-4");
        assert!(err.is_client_error());
        assert_eq!(err.status_code(), 404);
    }

    #[test]
    fn test_error_display() {
        let err = SpectreError::timeout("llm request", 30);
        assert_eq!(
            err.to_string(),
            "Operation timed out after 30s: llm request"
        );
    }
}
