use contract::{
    BuildSummaryRequest, BuildSummaryResponse, CapabilityError, CapabilityRequest,
    CapabilityResponse, ExplainRustRequest, ExplainRustResponse,
};
use std::error::Error;

// ---------------------------------------------------------------------------
// Runtime error
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("provider error: {0}")]
    Provider(Box<dyn Error>),

    #[error("invalid response from provider")]
    InvalidResponse,

    #[error("capability error: {0}")]
    Capability(#[from] CapabilityError),
}

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

/// Core provider interface. Every backend implements this trait.
pub trait CapabilityProvider {
    fn execute(&self, request: CapabilityRequest) -> Result<CapabilityResponse, RuntimeError>;
}

// ---------------------------------------------------------------------------
// Ergonomic extension methods
// ---------------------------------------------------------------------------

pub trait CapabilityProviderExt: CapabilityProvider {
    fn build_summary(
        &self,
        request: BuildSummaryRequest,
    ) -> Result<BuildSummaryResponse, RuntimeError> {
        match self.execute(CapabilityRequest::BuildSummary(request))? {
            CapabilityResponse::BuildSummary(resp) => Ok(resp),
            _other => Err(RuntimeError::InvalidResponse),
        }
    }

    fn explain_rust(
        &self,
        request: ExplainRustRequest,
    ) -> Result<ExplainRustResponse, RuntimeError> {
        match self.execute(CapabilityRequest::ExplainRust(request))? {
            CapabilityResponse::ExplainRust(resp) => Ok(resp),
            _other => Err(RuntimeError::InvalidResponse),
        }
    }
}

impl<T: CapabilityProvider> CapabilityProviderExt for T {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use contract::{BuildSummaryResponse, ExplainRustResponse, RequestMetadata};

    struct EchoProvider;

    impl CapabilityProvider for EchoProvider {
        fn execute(&self, request: CapabilityRequest) -> Result<CapabilityResponse, RuntimeError> {
            match request {
                CapabilityRequest::BuildSummary(req) => {
                    Ok(CapabilityResponse::BuildSummary(BuildSummaryResponse {
                        success: req.exit_code == 0,
                        summary: format!("exit {}", req.exit_code),
                        recommendation: None,
                    }))
                }
                CapabilityRequest::ExplainRust(req) => {
                    Ok(CapabilityResponse::ExplainRust(ExplainRustResponse {
                        explanation: format!("about: {}", req.question),
                    }))
                }
            }
        }
    }

    #[test]
    fn build_summary_delegates_correctly() {
        let provider = EchoProvider;
        let resp = provider
            .build_summary(BuildSummaryRequest {
                metadata: RequestMetadata::new(),
                command: "cargo build".into(),
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
            })
            .unwrap();
        assert!(resp.success);
        assert_eq!(resp.summary, "exit 0");
    }

    #[test]
    fn explain_rust_delegates_correctly() {
        let provider = EchoProvider;
        let resp = provider
            .explain_rust(ExplainRustRequest {
                metadata: RequestMetadata::new(),
                source: "let x = 1;".into(),
                question: "what is x?".into(),
            })
            .unwrap();
        assert_eq!(resp.explanation, "about: what is x?");
    }

    #[test]
    fn build_summary_failure_case() {
        let provider = EchoProvider;
        let resp = provider
            .build_summary(BuildSummaryRequest {
                metadata: RequestMetadata::new(),
                command: "cargo build".into(),
                exit_code: 1,
                stdout: String::new(),
                stderr: "error".into(),
            })
            .unwrap();
        assert!(!resp.success);
    }

    #[test]
    fn runtime_error_display() {
        let e = RuntimeError::InvalidResponse;
        assert_eq!(e.to_string(), "invalid response from provider");

        let e = RuntimeError::Capability(CapabilityError::InvalidRequest);
        assert_eq!(e.to_string(), "capability error: invalid request");
    }
}
