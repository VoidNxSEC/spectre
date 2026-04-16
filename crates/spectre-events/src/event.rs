//! Event schema definitions
//!
//! Defines the structure of all events in the SPECTRE Fleet.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use spectre_core::{CorrelationId, ServiceId, TraceId};
use uuid::Uuid;

/// Event type enumeration
///
/// All events follow the pattern: `<category>.<action>.v<version>`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventType {
    // LLM Gateway events
    #[serde(rename = "llm.request.v1")]
    LlmRequest,
    #[serde(rename = "llm.response.v1")]
    LlmResponse,

    // ML Inference events
    #[serde(rename = "inference.request.v1")]
    InferenceRequest,
    #[serde(rename = "inference.response.v1")]
    InferenceResponse,
    #[serde(rename = "vram.status.v1")]
    VramStatus,

    // Analysis events
    #[serde(rename = "analysis.request.v1")]
    AnalysisRequest,
    #[serde(rename = "analysis.response.v1")]
    AnalysisResponse,
    #[serde(rename = "analysis.report.v1")]
    AnalysisReport,

    // RAG events
    #[serde(rename = "rag.index.v1")]
    RagIndex,
    #[serde(rename = "rag.query.v1")]
    RagQuery,
    #[serde(rename = "document.indexed.v1")]
    DocumentIndexed,

    // System events
    #[serde(rename = "system.metrics.v1")]
    SystemMetrics,
    #[serde(rename = "system.log.v1")]
    SystemLog,

    // Hyprland events
    #[serde(rename = "hyprland.window.v1")]
    HyprlandWindow,
    #[serde(rename = "hyprland.workspace.v1")]
    HyprlandWorkspace,

    // Cost tracking (FinOps)
    #[serde(rename = "cost.incurred.v1")]
    CostIncurred,

    // Agent orchestration
    #[serde(rename = "task.assigned.v1")]
    TaskAssigned,
    #[serde(rename = "task.result.v1")]
    TaskResult,

    // Governance
    #[serde(rename = "governance.proposal.v1")]
    GovernanceProposal,
    #[serde(rename = "governance.vote.v1")]
    GovernanceVote,
    #[serde(rename = "quality.report.v1")]
    QualityReport,

    // Agent commands
    #[serde(rename = "agent.command.v1")]
    AgentCommand,

    // Network events (owasaka sensor layer)
    #[serde(rename = "network.dns.query.v1")]
    NetworkDnsQuery,
    #[serde(rename = "network.dns.threat.v1")]
    NetworkDnsThreat,
    #[serde(rename = "network.asset.discovered.v1")]
    NetworkAssetDiscovered,
    #[serde(rename = "network.service.detected.v1")]
    NetworkServiceDetected,
    #[serde(rename = "network.topology.updated.v1")]
    NetworkTopologyUpdated,

    // Phantom-stack domain events
    #[serde(rename = "ingest.file.created.v1")]
    IngestFileCreated,
    #[serde(rename = "ingest.file.sanitized.v1")]
    IngestFileSanitized,
    #[serde(rename = "cognition.query.received.v1")]
    CognitionQueryReceived,
    #[serde(rename = "cognition.insight.generated.v1")]
    CognitionInsightGenerated,

    // Generic/Custom event
    #[serde(rename = "custom")]
    Custom(String),
}

impl EventType {
    /// Get the NATS subject for this event type
    pub fn subject(&self) -> String {
        match self {
            Self::Custom(s) => s.clone(),
            other => {
                // Serialize to get the string representation
                serde_json::to_string(other)
                    .unwrap()
                    .trim_matches('"')
                    .to_string()
            }
        }
    }

    /// Check if this is a request event (expects response)
    pub fn is_request(&self) -> bool {
        matches!(
            self,
            Self::LlmRequest | Self::InferenceRequest | Self::AnalysisRequest | Self::RagQuery
        )
    }

    /// Get the corresponding response event type
    pub fn response_type(&self) -> Option<EventType> {
        match self {
            Self::LlmRequest => Some(Self::LlmResponse),
            Self::InferenceRequest => Some(Self::InferenceResponse),
            Self::AnalysisRequest => Some(Self::AnalysisResponse),
            _ => None,
        }
    }
}

/// Event payload (JSON)
pub type EventPayload = serde_json::Value;

/// Event metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    /// OpenTelemetry trace ID
    pub trace_id: TraceId,

    /// Service that authenticated this request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authenticated_by: Option<String>,

    /// Cost allocation tag (for FinOps)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_allocation_tag: Option<String>,

    /// Additional custom metadata
    #[serde(flatten)]
    pub custom: serde_json::Map<String, serde_json::Value>,
}

impl Default for EventMetadata {
    fn default() -> Self {
        Self {
            trace_id: TraceId::generate(),
            authenticated_by: None,
            cost_allocation_tag: None,
            custom: serde_json::Map::new(),
        }
    }
}

/// SPECTRE Event
///
/// The fundamental unit of communication in the SPECTRE Fleet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique event ID
    pub event_id: Uuid,

    /// Event type (subject)
    pub event_type: EventType,

    /// Timestamp (UTC)
    pub timestamp: DateTime<Utc>,

    /// Source service that published this event
    pub source_service: ServiceId,

    /// Target service (optional, for directed messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_service: Option<ServiceId>,

    /// Correlation ID (links related events)
    pub correlation_id: CorrelationId,

    /// Event payload (JSON)
    pub payload: EventPayload,

    /// Metadata (tracing, auth, cost allocation)
    pub metadata: EventMetadata,
}

impl Event {
    /// Create a new event
    pub fn new(event_type: EventType, source_service: ServiceId, payload: EventPayload) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            event_type,
            timestamp: Utc::now(),
            source_service,
            target_service: None,
            correlation_id: CorrelationId::generate(),
            payload,
            metadata: EventMetadata::default(),
        }
    }

    /// Create a new event with explicit correlation ID
    pub fn with_correlation(
        event_type: EventType,
        source_service: ServiceId,
        correlation_id: CorrelationId,
        payload: EventPayload,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            event_type,
            timestamp: Utc::now(),
            source_service,
            target_service: None,
            correlation_id,
            payload,
            metadata: EventMetadata::default(),
        }
    }

    /// Set the target service
    pub fn with_target(mut self, target: ServiceId) -> Self {
        self.target_service = Some(target);
        self
    }

    /// Set the trace ID
    pub fn with_trace_id(mut self, trace_id: TraceId) -> Self {
        self.metadata.trace_id = trace_id;
        self
    }

    /// Set authenticated_by
    pub fn with_auth(mut self, service: impl Into<String>) -> Self {
        self.metadata.authenticated_by = Some(service.into());
        self
    }

    /// Set cost allocation tag
    pub fn with_cost_tag(mut self, tag: impl Into<String>) -> Self {
        self.metadata.cost_allocation_tag = Some(tag.into());
        self
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> spectre_core::Result<String> {
        serde_json::to_string(self).map_err(|e| {
            spectre_core::SpectreError::serialization(format!("Failed to serialize event: {}", e))
        })
    }

    /// Deserialize from JSON
    pub fn from_json(json: &str) -> spectre_core::Result<Self> {
        serde_json::from_str(json).map_err(|e| {
            spectre_core::SpectreError::serialization(format!("Failed to deserialize event: {}", e))
        })
    }

    /// Get the NATS subject for this event
    pub fn subject(&self) -> String {
        self.event_type.subject()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = Event::new(
            EventType::SystemMetrics,
            ServiceId::new("agent-os"),
            serde_json::json!({"cpu": 45.2}),
        );

        assert_eq!(event.event_type, EventType::SystemMetrics);
        assert_eq!(event.source_service.as_str(), "agent-os");
        assert!(event.target_service.is_none());
    }

    #[test]
    fn test_event_serialization() {
        let event = Event::new(
            EventType::LlmRequest,
            ServiceId::new("rag-service"),
            serde_json::json!({"prompt": "test"}),
        );

        let json = event.to_json().unwrap();
        let deserialized = Event::from_json(&json).unwrap();

        assert_eq!(event.event_id, deserialized.event_id);
        assert_eq!(event.event_type, deserialized.event_type);
    }

    #[test]
    fn test_event_subject() {
        let event = Event::new(
            EventType::LlmRequest,
            ServiceId::new("test"),
            serde_json::json!({}),
        );

        assert_eq!(event.subject(), "llm.request.v1");
    }

    #[test]
    fn test_event_type_request_response() {
        assert!(EventType::LlmRequest.is_request());
        assert!(!EventType::SystemMetrics.is_request());

        assert_eq!(
            EventType::LlmRequest.response_type(),
            Some(EventType::LlmResponse)
        );
    }

    #[test]
    fn test_network_event_subjects() {
        assert_eq!(EventType::NetworkDnsQuery.subject(), "network.dns.query.v1");
        assert_eq!(
            EventType::NetworkDnsThreat.subject(),
            "network.dns.threat.v1"
        );
        assert_eq!(
            EventType::NetworkAssetDiscovered.subject(),
            "network.asset.discovered.v1"
        );
        assert_eq!(
            EventType::NetworkServiceDetected.subject(),
            "network.service.detected.v1"
        );
        assert_eq!(
            EventType::NetworkTopologyUpdated.subject(),
            "network.topology.updated.v1"
        );
    }

    #[test]
    fn test_phantom_stack_event_subjects() {
        assert_eq!(
            EventType::IngestFileCreated.subject(),
            "ingest.file.created.v1"
        );
        assert_eq!(
            EventType::IngestFileSanitized.subject(),
            "ingest.file.sanitized.v1"
        );
        assert_eq!(
            EventType::CognitionQueryReceived.subject(),
            "cognition.query.received.v1"
        );
        assert_eq!(
            EventType::CognitionInsightGenerated.subject(),
            "cognition.insight.generated.v1"
        );
    }

    #[test]
    fn test_network_event_serialization() {
        let event = Event::new(
            EventType::NetworkAssetDiscovered,
            ServiceId::new("owasaka"),
            serde_json::json!({"ip": "192.168.1.10", "mac": "aa:bb:cc:dd:ee:ff"}),
        );

        let json = event.to_json().unwrap();
        let deserialized = Event::from_json(&json).unwrap();

        assert_eq!(deserialized.event_type, EventType::NetworkAssetDiscovered);
        assert_eq!(deserialized.subject(), "network.asset.discovered.v1");
    }
}
