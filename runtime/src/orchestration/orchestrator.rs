use std::sync::Arc;

use crate::assembler::PromptAssembler;
use crate::capability_parser::CapabilityResponseParser;
use crate::context::ContextSource;
use crate::local_model::InferenceResponse;
use crate::parser::OutputParser;
use crate::{CapabilityProvider, RuntimeError};

use super::request::{OrchestrationRequest, OrchestrationResponse};
use contract::{CapabilityRequest, CapabilityResponse};

pub struct Orchestrator {
    provider: Arc<dyn CapabilityProvider + Send + Sync>,
    context_source: Arc<dyn ContextSource>,
    prompt_assembler: Arc<dyn PromptAssembler + Send + Sync>,
    output_parser: Arc<dyn OutputParser + Send + Sync>,
    capability_parser: Arc<dyn CapabilityResponseParser + Send + Sync>,
}

impl Orchestrator {
    pub fn new(
        provider: Arc<dyn CapabilityProvider + Send + Sync>,
        context_source: Arc<dyn ContextSource>,
        prompt_assembler: Arc<dyn PromptAssembler + Send + Sync>,
        output_parser: Arc<dyn OutputParser + Send + Sync>,
        capability_parser: Arc<dyn CapabilityResponseParser + Send + Sync>,
    ) -> Self {
        Self {
            provider,
            context_source,
            prompt_assembler,
            output_parser,
            capability_parser,
        }
    }

    pub fn execute(
        &self,
        request: OrchestrationRequest,
    ) -> Result<OrchestrationResponse, RuntimeError> {
        let context = match request.context {
            Some(ctx) => ctx,
            None => self.context_source.context().unwrap_or_default(),
        };

        let prompt = self
            .prompt_assembler
            .assemble(&context, &request.capability);

        let original = request.capability.clone();
        let enriched = inject_prompt(request.capability, &prompt);

        let response = self.provider.execute(enriched)?;

        let raw = extract_raw_text(&response);
        let model_output = self.output_parser.parse(&InferenceResponse {
            text: raw.clone(),
            profile: None,
        });

        let capability = self.capability_parser.parse(&model_output, &original);

        Ok(OrchestrationResponse {
            capability,
            raw_output: model_output.raw,
            warnings: model_output.warnings,
        })
    }

    /// Get workspace context directly without LLM inference.
    /// Used by workspace_context tool to return raw context.
    pub fn context(&self) -> Result<crate::context::InferenceContext, RuntimeError> {
        self.context_source.context()
    }
}

fn inject_prompt(capability: CapabilityRequest, prompt: &str) -> CapabilityRequest {
    match capability {
        CapabilityRequest::BuildSummary(mut req) => {
            req.prompt = prompt.to_string();
            CapabilityRequest::BuildSummary(req)
        }
        CapabilityRequest::ExplainRust(mut req) => {
            req.prompt = prompt.to_string();
            CapabilityRequest::ExplainRust(req)
        }
    }
}

fn extract_raw_text(response: &CapabilityResponse) -> String {
    match response {
        CapabilityResponse::BuildSummary(resp) => resp.summary.clone(),
        CapabilityResponse::ExplainRust(resp) => resp.explanation.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembler::DefaultPromptAssembler;
    use crate::capability_parser::CapabilityResponseParser;
    use crate::context::{InferenceContext, NullContextSource, WorkspaceStatus};
    use crate::parser::{ModelOutput, OutputFormat};
    use contract::*;

    struct MockProvider;

    impl CapabilityProvider for MockProvider {
        fn info(&self) -> ProviderInfo {
            ProviderInfo {
                name: "mock".into(),
                version: "0.1.0".into(),
                supported_capabilities: vec![
                    CapabilityKind::BuildSummary,
                    CapabilityKind::ExplainRust,
                ],
            }
        }

        fn health(&self) -> contract::ProviderHealth {
            contract::ProviderHealth::Healthy
        }

        fn execute(&self, request: CapabilityRequest) -> Result<CapabilityResponse, RuntimeError> {
            match request {
                CapabilityRequest::BuildSummary(req) => {
                    Ok(CapabilityResponse::BuildSummary(BuildSummaryResponse {
                        success: req.exit_code == 0,
                        summary: format!("Build output: {}", req.stderr),
                        recommendation: None,
                    }))
                }
                CapabilityRequest::ExplainRust(req) => {
                    Ok(CapabilityResponse::ExplainRust(ExplainRustResponse {
                        explanation: format!("Explanation of: {}", req.question),
                    }))
                }
            }
        }
    }

    struct PassthroughParser;

    impl CapabilityResponseParser for PassthroughParser {
        fn parse(&self, output: &ModelOutput, request: &CapabilityRequest) -> CapabilityResponse {
            match request {
                CapabilityRequest::BuildSummary(_) => {
                    CapabilityResponse::BuildSummary(BuildSummaryResponse {
                        success: true,
                        summary: output.raw.clone(),
                        recommendation: None,
                    })
                }
                CapabilityRequest::ExplainRust(_) => {
                    CapabilityResponse::ExplainRust(ExplainRustResponse {
                        explanation: output.raw.clone(),
                    })
                }
            }
        }
    }

    fn test_orchestrator() -> Orchestrator {
        Orchestrator::new(
            Arc::new(MockProvider),
            Arc::new(NullContextSource),
            Arc::new(DefaultPromptAssembler),
            Arc::new(crate::parser::DefaultOutputParser),
            Arc::new(PassthroughParser),
        )
    }

    #[test]
    fn orchestrator_build_summary() {
        let orch = test_orchestrator();
        let request = OrchestrationRequest {
            capability: CapabilityRequest::BuildSummary(BuildSummaryRequest {
                metadata: RequestMetadata::new(),
                command: "cargo build".into(),
                exit_code: 1,
                stdout: String::new(),
                stderr: "error[E0599]: no method named `foo`".into(),
                prompt: String::new(),
                temperature: 0.1,
                max_tokens: 256,
            }),
            context: None,
        };

        let resp = orch.execute(request).unwrap();
        assert!(!resp.text().is_empty());
        assert!(resp.warnings.is_empty());
    }

    #[test]
    fn orchestrator_explain_rust() {
        let orch = test_orchestrator();
        let request = OrchestrationRequest {
            capability: CapabilityRequest::ExplainRust(ExplainRustRequest {
                metadata: RequestMetadata::new(),
                source: "let x = 1;".into(),
                question: "What is x?".into(),
                prompt: String::new(),
                temperature: 0.1,
                max_tokens: 256,
            }),
            context: None,
        };

        let resp = orch.execute(request).unwrap();
        assert!(!resp.text().is_empty());
    }

    #[test]
    fn orchestrator_with_no_context() {
        let orch = test_orchestrator();
        let request = OrchestrationRequest {
            capability: CapabilityRequest::BuildSummary(BuildSummaryRequest {
                metadata: RequestMetadata::new(),
                command: "test".into(),
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                prompt: String::new(),
                temperature: 0.1,
                max_tokens: 256,
            }),
            context: None,
        };

        let resp = orch.execute(request).unwrap();
        assert!(!resp.text().is_empty());
    }

    #[test]
    fn orchestrator_with_injected_context() {
        let orch = test_orchestrator();
        let context = InferenceContext {
            recent_changes: vec!["src/main.rs".into()],
            workspace_status: WorkspaceStatus::Modified,
            file_count: 42,
            hash: 12345,
            git_branch: None,
            git_dirty: false,
            git_modified_count: 0,
            tracked_files: 50,
        };

        let request = OrchestrationRequest {
            capability: CapabilityRequest::BuildSummary(BuildSummaryRequest {
                metadata: RequestMetadata::new(),
                command: "cargo build".into(),
                exit_code: 1,
                stdout: String::new(),
                stderr: "error".into(),
                prompt: String::new(),
                temperature: 0.1,
                max_tokens: 256,
            }),
            context: Some(context),
        };

        let resp = orch.execute(request).unwrap();
        assert!(!resp.text().is_empty());
    }

    #[test]
    fn orchestrator_provider_failure() {
        struct FailingProvider;

        impl CapabilityProvider for FailingProvider {
            fn info(&self) -> ProviderInfo {
                ProviderInfo {
                    name: "failing".into(),
                    version: "0.1.0".into(),
                    supported_capabilities: vec![CapabilityKind::BuildSummary],
                }
            }

            fn health(&self) -> contract::ProviderHealth {
                contract::ProviderHealth::Unhealthy {
                    reason: "test".into(),
                }
            }

            fn execute(
                &self,
                _request: CapabilityRequest,
            ) -> Result<CapabilityResponse, RuntimeError> {
                Err(RuntimeError::Provider("intentional failure".into()))
            }
        }

        let orch = Orchestrator::new(
            Arc::new(FailingProvider),
            Arc::new(NullContextSource),
            Arc::new(DefaultPromptAssembler),
            Arc::new(crate::parser::DefaultOutputParser),
            Arc::new(PassthroughParser),
        );

        let request = OrchestrationRequest {
            capability: CapabilityRequest::BuildSummary(BuildSummaryRequest {
                metadata: RequestMetadata::new(),
                command: "test".into(),
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                prompt: String::new(),
                temperature: 0.1,
                max_tokens: 256,
            }),
            context: None,
        };

        let result = orch.execute(request);
        assert!(result.is_err());
    }

    #[test]
    fn orchestrator_context_source_failure_falls_back_to_default() {
        struct FailingContextSource;

        impl ContextSource for FailingContextSource {
            fn context(&self) -> Result<InferenceContext, RuntimeError> {
                Err(RuntimeError::Provider("context unavailable".into()))
            }
        }

        let orch = Orchestrator::new(
            Arc::new(MockProvider),
            Arc::new(FailingContextSource),
            Arc::new(DefaultPromptAssembler),
            Arc::new(crate::parser::DefaultOutputParser),
            Arc::new(PassthroughParser),
        );

        let request = OrchestrationRequest {
            capability: CapabilityRequest::BuildSummary(BuildSummaryRequest {
                metadata: RequestMetadata::new(),
                command: "test".into(),
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                prompt: String::new(),
                temperature: 0.1,
                max_tokens: 256,
            }),
            context: None,
        };

        let resp = orch.execute(request).unwrap();
        assert!(!resp.text().is_empty());
    }

    #[test]
    fn orchestrator_context_returns_default() {
        let orch = test_orchestrator();
        let ctx = orch.context().unwrap();
        assert_eq!(ctx, InferenceContext::default());
    }

    #[test]
    fn orchestrator_context_with_workspace() {
        struct TestContextSource;

        impl ContextSource for TestContextSource {
            fn context(&self) -> Result<InferenceContext, RuntimeError> {
                Ok(InferenceContext {
                    recent_changes: vec!["src/main.rs".into()],
                    workspace_status: WorkspaceStatus::Modified,
                    file_count: 10,
                    hash: 12345,
                    git_branch: Some("main".into()),
                    git_dirty: true,
                    git_modified_count: 3,
                    tracked_files: 15,
                })
            }
        }

        let orch = Orchestrator::new(
            Arc::new(MockProvider),
            Arc::new(TestContextSource),
            Arc::new(DefaultPromptAssembler),
            Arc::new(crate::parser::DefaultOutputParser),
            Arc::new(PassthroughParser),
        );

        let ctx = orch.context().unwrap();
        assert_eq!(ctx.workspace_status, WorkspaceStatus::Modified);
        assert_eq!(ctx.file_count, 10);
        assert_eq!(ctx.git_branch, Some("main".into()));
        assert!(ctx.git_dirty);
        assert_eq!(ctx.git_modified_count, 3);
        assert_eq!(ctx.recent_changes, vec!["src/main.rs".to_string()]);
    }

    #[test]
    fn orchestrator_context_failure_returns_error() {
        struct FailingContextSource;

        impl ContextSource for FailingContextSource {
            fn context(&self) -> Result<InferenceContext, RuntimeError> {
                Err(RuntimeError::Provider("context unavailable".into()))
            }
        }

        let orch = Orchestrator::new(
            Arc::new(MockProvider),
            Arc::new(FailingContextSource),
            Arc::new(DefaultPromptAssembler),
            Arc::new(crate::parser::DefaultOutputParser),
            Arc::new(PassthroughParser),
        );

        let result = orch.context();
        assert!(result.is_err());
    }
}
