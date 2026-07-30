use contract::{BuildSummaryResponse, CapabilityRequest, CapabilityResponse, ExplainRustResponse};
use runtime::capability_parser::CapabilityResponseParser;
use runtime::parser::{ModelOutput, OutputFormat};

// ---------------------------------------------------------------------------
// BuildSummaryParser
//
// Extracts BuildSummaryResponse from model output. Falls back
// to raw text on non-JSON output.
// ---------------------------------------------------------------------------

pub struct BuildSummaryParser;

impl CapabilityResponseParser for BuildSummaryParser {
    fn parse(&self, output: &ModelOutput, _request: &CapabilityRequest) -> CapabilityResponse {
        let mut warnings = output.warnings.clone();

        match &output.format {
            OutputFormat::Json(v) => {
                let success = v
                    .get("success")
                    .and_then(|s| s.as_bool())
                    .unwrap_or_else(|| {
                        warnings.push("missing required field: success".into());
                        false
                    });

                let summary = v
                    .get("summary")
                    .and_then(|s| s.as_str())
                    .unwrap_or_else(|| {
                        warnings.push("missing required field: summary".into());
                        ""
                    });

                let recommendation = v
                    .get("recommendation")
                    .and_then(|s| s.as_str())
                    .map(String::from);

                CapabilityResponse::BuildSummary(BuildSummaryResponse {
                    success,
                    summary: summary.into(),
                    recommendation,
                })
            }
            OutputFormat::Text(t) => CapabilityResponse::BuildSummary(BuildSummaryResponse {
                success: true,
                summary: t.clone(),
                recommendation: None,
            }),
            OutputFormat::Empty => CapabilityResponse::BuildSummary(BuildSummaryResponse {
                success: false,
                summary: String::new(),
                recommendation: None,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// ExplainRustParser
//
// Extracts ExplainRustResponse from model output. Falls back
// to raw text on non-JSON output.
// ---------------------------------------------------------------------------

pub struct ExplainRustParser;

impl CapabilityResponseParser for ExplainRustParser {
    fn parse(&self, output: &ModelOutput, _request: &CapabilityRequest) -> CapabilityResponse {
        let mut warnings = output.warnings.clone();

        match &output.format {
            OutputFormat::Json(v) => {
                let explanation = v
                    .get("explanation")
                    .and_then(|s| s.as_str())
                    .unwrap_or_else(|| {
                        warnings.push("missing required field: explanation".into());
                        ""
                    });

                CapabilityResponse::ExplainRust(ExplainRustResponse {
                    explanation: explanation.into(),
                })
            }
            OutputFormat::Text(t) => CapabilityResponse::ExplainRust(ExplainRustResponse {
                explanation: t.clone(),
            }),
            OutputFormat::Empty => CapabilityResponse::ExplainRust(ExplainRustResponse {
                explanation: String::new(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use contract::RequestMetadata;

    #[test]
    fn build_summary_json_success() {
        let parser = BuildSummaryParser;
        let output = ModelOutput {
            raw: r#"{"success": true, "summary": "Build passed", "recommendation": "none"}"#.into(),
            format: OutputFormat::Json(serde_json::json!({
                "success": true,
                "summary": "Build passed",
                "recommendation": "none"
            })),
            warnings: vec![],
        };
        let request = CapabilityRequest::BuildSummary(contract::BuildSummaryRequest {
            metadata: RequestMetadata::new(),
            command: "cargo build".into(),
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
                assert!(resp.success);
                assert_eq!(resp.summary, "Build passed");
                assert_eq!(resp.recommendation, Some("none".into()));
            }
            _ => panic!("expected BuildSummary"),
        }
    }

    #[test]
    fn build_summary_json_missing_fields() {
        let parser = BuildSummaryParser;
        let output = ModelOutput {
            raw: "{}".into(),
            format: OutputFormat::Json(serde_json::json!({})),
            warnings: vec![],
        };
        let request = CapabilityRequest::BuildSummary(contract::BuildSummaryRequest {
            metadata: RequestMetadata::new(),
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
                assert!(!resp.success);
                assert_eq!(resp.summary, "");
            }
            _ => panic!("expected BuildSummary"),
        }
    }

    #[test]
    fn build_summary_text_fallback() {
        let parser = BuildSummaryParser;
        let output = ModelOutput {
            raw: "Build succeeded with warnings".into(),
            format: OutputFormat::Text("Build succeeded with warnings".into()),
            warnings: vec![],
        };
        let request = CapabilityRequest::BuildSummary(contract::BuildSummaryRequest {
            metadata: RequestMetadata::new(),
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
                assert!(resp.success);
                assert_eq!(resp.summary, "Build succeeded with warnings");
                assert_eq!(resp.recommendation, None);
            }
            _ => panic!("expected BuildSummary"),
        }
    }

    #[test]
    fn build_summary_empty() {
        let parser = BuildSummaryParser;
        let output = ModelOutput {
            raw: String::new(),
            format: OutputFormat::Empty,
            warnings: vec![],
        };
        let request = CapabilityRequest::BuildSummary(contract::BuildSummaryRequest {
            metadata: RequestMetadata::new(),
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
                assert!(!resp.success);
                assert_eq!(resp.summary, "");
            }
            _ => panic!("expected BuildSummary"),
        }
    }

    #[test]
    fn explain_rust_json() {
        let parser = ExplainRustParser;
        let output = ModelOutput {
            raw: r#"{"explanation": "This is a main function"}"#.into(),
            format: OutputFormat::Json(serde_json::json!({
                "explanation": "This is a main function"
            })),
            warnings: vec![],
        };
        let request = CapabilityRequest::ExplainRust(contract::ExplainRustRequest {
            metadata: RequestMetadata::new(),
            source: "fn main() {}".into(),
            question: "what does this do?".into(),
            prompt: "explain".into(),
            temperature: 0.2,
            max_tokens: 1024,
        });

        let response = parser.parse(&output, &request);
        match response {
            CapabilityResponse::ExplainRust(resp) => {
                assert_eq!(resp.explanation, "This is a main function");
            }
            _ => panic!("expected ExplainRust"),
        }
    }

    #[test]
    fn explain_rust_text_fallback() {
        let parser = ExplainRustParser;
        let output = ModelOutput {
            raw: "This function prints hello world".into(),
            format: OutputFormat::Text("This function prints hello world".into()),
            warnings: vec![],
        };
        let request = CapabilityRequest::ExplainRust(contract::ExplainRustRequest {
            metadata: RequestMetadata::new(),
            source: "fn main() {}".into(),
            question: "what does this do?".into(),
            prompt: "explain".into(),
            temperature: 0.2,
            max_tokens: 1024,
        });

        let response = parser.parse(&output, &request);
        match response {
            CapabilityResponse::ExplainRust(resp) => {
                assert_eq!(resp.explanation, "This function prints hello world");
            }
            _ => panic!("expected ExplainRust"),
        }
    }

    #[test]
    fn explain_rust_empty() {
        let parser = ExplainRustParser;
        let output = ModelOutput {
            raw: String::new(),
            format: OutputFormat::Empty,
            warnings: vec![],
        };
        let request = CapabilityRequest::ExplainRust(contract::ExplainRustRequest {
            metadata: RequestMetadata::new(),
            source: "fn main() {}".into(),
            question: "what does this do?".into(),
            prompt: "explain".into(),
            temperature: 0.2,
            max_tokens: 1024,
        });

        let response = parser.parse(&output, &request);
        match response {
            CapabilityResponse::ExplainRust(resp) => {
                assert_eq!(resp.explanation, "");
            }
            _ => panic!("expected ExplainRust"),
        }
    }

    #[test]
    fn build_summary_json_in_fences() {
        let parser = BuildSummaryParser;
        let output = ModelOutput {
            raw: "```json\n{\"success\": true, \"summary\": \"ok\"}\n```".into(),
            format: OutputFormat::Json(serde_json::json!({
                "success": true,
                "summary": "ok"
            })),
            warnings: vec![],
        };
        let request = CapabilityRequest::BuildSummary(contract::BuildSummaryRequest {
            metadata: RequestMetadata::new(),
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
                assert!(resp.success);
                assert_eq!(resp.summary, "ok");
            }
            _ => panic!("expected BuildSummary"),
        }
    }
}
