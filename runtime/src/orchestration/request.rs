use contract::{CapabilityRequest, CapabilityResponse};

use crate::context::InferenceContext;

pub struct OrchestrationRequest {
    pub capability: CapabilityRequest,
    pub context: Option<InferenceContext>,
}

pub struct OrchestrationResponse {
    pub capability: CapabilityResponse,
    pub raw_output: String,
    pub warnings: Vec<String>,
}

impl OrchestrationResponse {
    pub fn text(&self) -> &str {
        match &self.capability {
            CapabilityResponse::BuildSummary(v) => &v.summary,
            CapabilityResponse::ExplainRust(v) => &v.explanation,
        }
    }
}
