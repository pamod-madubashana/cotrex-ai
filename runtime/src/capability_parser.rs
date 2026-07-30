use crate::parser::ModelOutput;
use contract::{CapabilityRequest, CapabilityResponse};

// ---------------------------------------------------------------------------
// CapabilityResponseParser
//
// Trait that extracts typed responses from ModelOutput. Defined
// in runtime so capability crates can implement it. Providers
// never see this trait.
// ---------------------------------------------------------------------------

pub trait CapabilityResponseParser {
    fn parse(&self, output: &ModelOutput, request: &CapabilityRequest) -> CapabilityResponse;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::OutputFormat;

    struct DummyParser;

    impl CapabilityResponseParser for DummyParser {
        fn parse(&self, output: &ModelOutput, request: &CapabilityRequest) -> CapabilityResponse {
            match request {
                CapabilityRequest::BuildSummary(_) => {
                    contract::CapabilityResponse::BuildSummary(contract::BuildSummaryResponse {
                        success: true,
                        summary: output.raw.clone(),
                        recommendation: None,
                    })
                }
                CapabilityRequest::ExplainRust(_) => {
                    contract::CapabilityResponse::ExplainRust(contract::ExplainRustResponse {
                        explanation: output.raw.clone(),
                    })
                }
            }
        }
    }

    #[test]
    fn dummy_parser_build_summary() {
        let parser = DummyParser;
        let output = ModelOutput {
            raw: "test output".into(),
            format: OutputFormat::Text("test output".into()),
            warnings: vec![],
        };
        let request = CapabilityRequest::BuildSummary(contract::BuildSummaryRequest {
            metadata: contract::RequestMetadata::new(),
            command: "test".into(),
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            prompt: "test".into(),
            temperature: 0.1,
            max_tokens: 100,
        });

        let response = parser.parse(&output, &request);
        match response {
            CapabilityResponse::BuildSummary(resp) => {
                assert_eq!(resp.summary, "test output");
            }
            _ => panic!("expected BuildSummary"),
        }
    }

    #[test]
    fn dummy_parser_explain_rust() {
        let parser = DummyParser;
        let output = ModelOutput {
            raw: "explanation".into(),
            format: OutputFormat::Text("explanation".into()),
            warnings: vec![],
        };
        let request = CapabilityRequest::ExplainRust(contract::ExplainRustRequest {
            metadata: contract::RequestMetadata::new(),
            source: "fn main() {}".into(),
            question: "what does this do?".into(),
            prompt: "explain".into(),
            temperature: 0.2,
            max_tokens: 1024,
        });

        let response = parser.parse(&output, &request);
        match response {
            CapabilityResponse::ExplainRust(resp) => {
                assert_eq!(resp.explanation, "explanation");
            }
            _ => panic!("expected ExplainRust"),
        }
    }
}
