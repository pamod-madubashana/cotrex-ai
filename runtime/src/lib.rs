use contract::{
    BuildSummaryRequest, BuildSummaryResponse, CapabilityError, CapabilityRequest,
    CapabilityResponse, ExplainRustRequest, ExplainRustResponse,
};
use std::error::Error;

pub mod adapter;
pub mod assembler;
pub mod capability_parser;
pub mod config;
pub mod context;
pub mod lifecycle;
pub mod local_model;
pub mod local_provider;
pub mod mock_model;
pub mod orchestrate;
pub mod parser;
pub use adapter::{RuntimeRequest, adapt_request, adapt_response};
pub use assembler::{DefaultPromptAssembler, PromptAssembler};
pub use capability_parser::CapabilityResponseParser;
pub use config::{
    ConfigError, EngineConfig, GlobalConfig, ModelConfig, ProjectConfig, ResolvedConfig,
};
pub use context::{ContextBuilder, DefaultContextBuilder, InferenceContext, WorkspaceStatus};
pub use lifecycle::ProviderLifecycle;
pub use local_model::{InferenceRequest, InferenceResponse, LocalModel, ModelInfo, Prompt};
pub use local_provider::LocalProvider;
pub use mock_model::MockLocalModel;
pub use orchestrate::execute_capability;
pub use parser::{DefaultOutputParser, ModelOutput, OutputFormat, OutputParser};

// ---------------------------------------------------------------------------
// Provider error
//
// Errors from model operations (load, infer, unload). Distinct from
// RuntimeError which handles protocol-level errors.
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("model error: {0}")]
    Model(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("state error: {0}")]
    State(#[from] contract::ProviderStateError),
}

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

    #[error("model error: {0}")]
    Model(String),
}

impl From<ProviderError> for RuntimeError {
    fn from(err: ProviderError) -> Self {
        match err {
            ProviderError::Model(msg) => RuntimeError::Model(msg),
            ProviderError::Config(msg) => RuntimeError::Model(msg),
            ProviderError::State(e) => RuntimeError::Model(e.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

/// Core provider interface. Every backend implements this trait.
///
/// Providers are `Send + Sync` — the runtime may hold `Arc<dyn CapabilityProvider>`
/// and share it across threads. Implementations must not rely on single-threaded
/// access.
///
/// Prompt building is private to each provider. The runtime never constructs
/// prompts. Different models (Llama, Qwen, Gemma) have different prompting
/// strategies, and those belong inside the provider.
///
/// The API is synchronous. Inference is CPU-bound; async would just move work
/// to `spawn_blocking` with no real benefit. If the runtime later needs async
/// orchestration, wrap synchronous providers without changing this contract.
pub trait CapabilityProvider: Send + Sync {
    /// Returns metadata about this provider.
    fn info(&self) -> contract::ProviderInfo;

    /// Reports provider health. Even if always `Healthy` today, this gives
    /// introspection for model-missing, weights-corrupted, OOM, etc.
    fn health(&self) -> contract::ProviderHealth;

    /// Execute a capability request and return the matching response variant.
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
    use contract::{
        BuildSummaryResponse, ExplainRustResponse, ProviderHealth, ProviderInfo, RequestMetadata,
    };

    struct EchoProvider;

    impl CapabilityProvider for EchoProvider {
        fn info(&self) -> ProviderInfo {
            ProviderInfo {
                name: "echo".into(),
                version: "0.1.0".into(),
                supported_capabilities: vec![
                    contract::CapabilityKind::BuildSummary,
                    contract::CapabilityKind::ExplainRust,
                ],
            }
        }

        fn health(&self) -> ProviderHealth {
            ProviderHealth::Healthy
        }

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
                prompt: "Summarize build: cargo build".into(),
                temperature: 0.1,
                max_tokens: 512,
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
                prompt: "Explain: what is x?\nlet x = 1;".into(),
                temperature: 0.2,
                max_tokens: 1024,
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
                prompt: "Summarize build: cargo build".into(),
                temperature: 0.1,
                max_tokens: 512,
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
