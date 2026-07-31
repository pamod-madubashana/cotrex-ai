use crate::adapter::adapt_request;
use crate::capability_parser::CapabilityResponseParser;
use crate::local_model::LocalModel;
use crate::parser::OutputParser;
use crate::{InferenceRequest, RuntimeError};
use contract::{CapabilityRequest, CapabilityResponse};

// ---------------------------------------------------------------------------
// execute_capability
//
// Single integration point for the full inference pipeline:
// model → output parser → capability parser → response.
//
// Callers must use this function. Manual composition is forbidden
// to prevent divergent inference paths.
// ---------------------------------------------------------------------------

pub fn execute_capability<M: LocalModel>(
    model: &M,
    output_parser: &dyn OutputParser,
    capability_parser: &dyn CapabilityResponseParser,
    request: CapabilityRequest,
) -> Result<CapabilityResponse, RuntimeError> {
    let runtime_req = adapt_request(request.clone())?;
    let inference_resp = model.infer(InferenceRequest {
        prompt: runtime_req.prompt,
        messages: vec![],
        temperature: runtime_req.temperature,
        max_tokens: runtime_req.max_tokens,
        token_callback: None,
    })?;
    let model_output = output_parser.parse(&inference_resp);
    Ok(capability_parser.parse(&model_output, &request))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability_parser::CapabilityResponseParser;
    use crate::local_model::LocalModel;
    use crate::parser::{ModelOutput, OutputFormat};
    use crate::{InferenceResponse, ProviderError, ResolvedConfig};
    use contract::{
        BuildSummaryRequest, BuildSummaryResponse, ExplainRustResponse, RequestMetadata,
    };

    // Mock model that returns canned output
    struct MockModel {
        output: String,
    }

    impl LocalModel for MockModel {
        fn load(&mut self, _config: &ResolvedConfig) -> Result<(), ProviderError> {
            Ok(())
        }

        fn infer(&self, _req: InferenceRequest) -> Result<InferenceResponse, ProviderError> {
            Ok(InferenceResponse {
                text: self.output.clone(),
                profile: None,
            })
        }

        fn unload(&mut self) -> Result<(), ProviderError> {
            Ok(())
        }

        fn info(&self) -> crate::ModelInfo {
            crate::ModelInfo {
                name: "mock".into(),
                version: "0.1.0".into(),
                backend: "test".into(),
            }
        }
    }

    // Test parser that returns whatever the mock output was
    struct TestOutputParser;

    impl OutputParser for TestOutputParser {
        fn parse(&self, response: &InferenceResponse) -> ModelOutput {
            ModelOutput {
                raw: response.text.clone(),
                format: OutputFormat::Text(response.text.clone()),
                warnings: vec![],
            }
        }
    }

    // Test capability parser that wraps raw text
    struct TestCapabilityParser;

    impl CapabilityResponseParser for TestCapabilityParser {
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

    #[test]
    fn execute_capability_full_flow() {
        let model = MockModel {
            output: "build passed".into(),
        };
        let output_parser = TestOutputParser;
        let capability_parser = TestCapabilityParser;

        let request = CapabilityRequest::BuildSummary(BuildSummaryRequest {
            metadata: RequestMetadata::new(),
            command: "cargo build".into(),
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            prompt: "summarize".into(),
            temperature: 0.1,
            max_tokens: 512,
        });

        let response = execute_capability(&model, &output_parser, &capability_parser, request);
        assert!(response.is_ok());

        match response.unwrap() {
            CapabilityResponse::BuildSummary(resp) => {
                assert!(resp.success);
                assert_eq!(resp.summary, "build passed");
            }
            _ => panic!("expected BuildSummary"),
        }
    }

    #[test]
    fn execute_capability_error_propagates() {
        struct FailingModel;

        impl LocalModel for FailingModel {
            fn load(&mut self, _config: &ResolvedConfig) -> Result<(), ProviderError> {
                Ok(())
            }

            fn infer(&self, _req: InferenceRequest) -> Result<InferenceResponse, ProviderError> {
                Err(ProviderError::Model("inference failed".into()))
            }

            fn unload(&mut self) -> Result<(), ProviderError> {
                Ok(())
            }

            fn info(&self) -> crate::ModelInfo {
                crate::ModelInfo {
                    name: "failing".into(),
                    version: "0.1.0".into(),
                    backend: "test".into(),
                }
            }
        }

        let model = FailingModel;
        let output_parser = TestOutputParser;
        let capability_parser = TestCapabilityParser;

        let request = CapabilityRequest::BuildSummary(BuildSummaryRequest {
            metadata: RequestMetadata::new(),
            command: "test".into(),
            exit_code: 1,
            stdout: String::new(),
            stderr: String::new(),
            prompt: "test".into(),
            temperature: 0.1,
            max_tokens: 100,
        });

        let result = execute_capability(&model, &output_parser, &capability_parser, request);
        assert!(result.is_err());
    }
}
