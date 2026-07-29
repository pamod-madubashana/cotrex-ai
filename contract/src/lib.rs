use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

pub mod provider_state;
pub use provider_state::{ProviderState, ProviderStateError};

// ---------------------------------------------------------------------------
// Protocol version
//
// Exact version match is required. A provider implementing protocol 1.0 will
// reject requests tagged 1.1 and vice versa. There is no negotiation, no
// downgrade, and no compatibility layer. Breaking changes are explicit and
// require a major version bump.
//
// Streaming responses are intentionally out of scope for Protocol v1.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

pub const PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion { major: 1, minor: 0 };

// ---------------------------------------------------------------------------
// Provider metadata
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderInfo {
    pub name: String,
    pub version: String,
    pub supported_capabilities: Vec<CapabilityKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapabilityKind {
    BuildSummary,
    ExplainRust,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderHealth {
    Healthy,
    Degraded { reason: &'static str },
    Unhealthy { reason: &'static str },
}

// ---------------------------------------------------------------------------
// Protocol error (contract-level, not runtime-level)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, thiserror::Error)]
pub enum CapabilityError {
    #[error("invalid request")]
    InvalidRequest,

    #[error("unsupported protocol version")]
    UnsupportedProtocolVersion,
}

// ---------------------------------------------------------------------------
// Request metadata
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RequestMetadata {
    pub id: Uuid,
    pub timestamp: SystemTime,
}

impl Default for RequestMetadata {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestMetadata {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: SystemTime::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildSummaryRequest {
    pub metadata: RequestMetadata,
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub prompt: String,
    pub temperature: f32,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExplainRustRequest {
    pub metadata: RequestMetadata,
    pub source: String,
    pub question: String,
    pub prompt: String,
    pub temperature: f32,
    pub max_tokens: u32,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildSummaryResponse {
    pub success: bool,
    pub summary: String,
    pub recommendation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExplainRustResponse {
    pub explanation: String,
}

// ---------------------------------------------------------------------------
// Capability protocol
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum CapabilityRequest {
    BuildSummary(BuildSummaryRequest),
    ExplainRust(ExplainRustRequest),
}

#[derive(Debug, Clone)]
pub enum CapabilityResponse {
    BuildSummary(BuildSummaryResponse),
    ExplainRust(ExplainRustResponse),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_version_is_1_0() {
        assert_eq!(PROTOCOL_VERSION, ProtocolVersion { major: 1, minor: 0 });
    }

    #[test]
    fn protocol_version_ord() {
        let v1 = ProtocolVersion { major: 1, minor: 0 };
        let v2 = ProtocolVersion { major: 1, minor: 1 };
        let v3 = ProtocolVersion { major: 2, minor: 0 };
        assert!(v1 < v2);
        assert!(v2 < v3);
    }

    #[test]
    fn build_summary_request_roundtrip() {
        let req = BuildSummaryRequest {
            metadata: RequestMetadata::new(),
            command: "cargo build".into(),
            exit_code: 1,
            stdout: String::new(),
            stderr: "error[E0308]: mismatched types".into(),
            prompt: "Summarize build: cargo build".into(),
            temperature: 0.1,
            max_tokens: 512,
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: BuildSummaryRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn build_summary_response_roundtrip() {
        let resp = BuildSummaryResponse {
            success: false,
            summary: "build failed".into(),
            recommendation: Some("check diagnostics".into()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: BuildSummaryResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, back);
    }

    #[test]
    fn build_summary_response_without_recommendation() {
        let resp = BuildSummaryResponse {
            success: true,
            summary: "ok".into(),
            recommendation: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: BuildSummaryResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, back);
        assert!(back.recommendation.is_none());
    }

    #[test]
    fn explain_rust_request_roundtrip() {
        let req = ExplainRustRequest {
            metadata: RequestMetadata::new(),
            source: "fn main() {}".into(),
            question: "what does this do?".into(),
            prompt: "Explain: what does this do?\nfn main() {}".into(),
            temperature: 0.2,
            max_tokens: 1024,
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: ExplainRustRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn explain_rust_response_roundtrip() {
        let resp = ExplainRustResponse {
            explanation: "prints hello".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: ExplainRustResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, back);
    }

    #[test]
    fn capability_request_is_clone() {
        let cap = CapabilityRequest::BuildSummary(BuildSummaryRequest {
            metadata: RequestMetadata::new(),
            command: "cargo test".into(),
            exit_code: 0,
            stdout: "ok".into(),
            stderr: String::new(),
            prompt: "test prompt".into(),
            temperature: 0.1,
            max_tokens: 100,
        });
        let _cloned = cap.clone();
    }

    #[test]
    fn capability_response_is_clone() {
        let resp = CapabilityResponse::BuildSummary(BuildSummaryResponse {
            success: true,
            summary: "ok".into(),
            recommendation: None,
        });
        let _cloned = resp.clone();
    }

    #[test]
    fn capability_error_display() {
        let e = CapabilityError::InvalidRequest;
        assert_eq!(e.to_string(), "invalid request");

        let e = CapabilityError::UnsupportedProtocolVersion;
        assert_eq!(e.to_string(), "unsupported protocol version");
    }
}
