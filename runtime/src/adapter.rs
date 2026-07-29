use crate::{InferenceResponse, Prompt, ProviderError, RuntimeError};
use contract::{BuildSummaryResponse, CapabilityRequest};

// ---------------------------------------------------------------------------
// RuntimeRequest
//
// Internal abstraction between CapabilityRequest and InferenceRequest.
// Keeps LocalProvider from knowing every protocol variant directly.
// ---------------------------------------------------------------------------

pub struct RuntimeRequest {
    pub prompt: Prompt,
    pub temperature: f32,
    pub max_tokens: u32,
}

// ---------------------------------------------------------------------------
// Adapter functions
//
// Translate between protocol types and inference types. The adapter
// does NOT construct prompts — prompts are already built by the
// Intelligence Brain.
// ---------------------------------------------------------------------------

pub fn adapt_request(request: CapabilityRequest) -> Result<RuntimeRequest, ProviderError> {
    match request {
        CapabilityRequest::BuildSummary(req) => Ok(RuntimeRequest {
            prompt: Prompt::new(req.prompt),
            temperature: req.temperature,
            max_tokens: req.max_tokens,
        }),
        CapabilityRequest::ExplainRust(req) => Ok(RuntimeRequest {
            prompt: Prompt::new(req.prompt),
            temperature: req.temperature,
            max_tokens: req.max_tokens,
        }),
    }
}

pub fn adapt_response(
    response: InferenceResponse,
) -> Result<contract::CapabilityResponse, RuntimeError> {
    Ok(contract::CapabilityResponse::BuildSummary(
        BuildSummaryResponse {
            success: true,
            summary: response.text,
            recommendation: None,
        },
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use contract::RequestMetadata;

    fn test_metadata() -> RequestMetadata {
        RequestMetadata::new()
    }

    #[test]
    fn adapt_build_summary_request() {
        let request = CapabilityRequest::BuildSummary(contract::BuildSummaryRequest {
            metadata: test_metadata(),
            command: "cargo build".into(),
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            prompt: "Summarize build: cargo build".into(),
            temperature: 0.1,
            max_tokens: 512,
        });
        let runtime_req = adapt_request(request).unwrap();
        assert_eq!(runtime_req.prompt.text, "Summarize build: cargo build");
        assert_eq!(runtime_req.temperature, 0.1);
        assert_eq!(runtime_req.max_tokens, 512);
    }

    #[test]
    fn adapt_explain_rust_request() {
        let request = CapabilityRequest::ExplainRust(contract::ExplainRustRequest {
            metadata: test_metadata(),
            source: "fn main() {}".into(),
            question: "what does this do?".into(),
            prompt: "Explain: what does this do?\nfn main() {}".into(),
            temperature: 0.2,
            max_tokens: 1024,
        });
        let runtime_req = adapt_request(request).unwrap();
        assert!(runtime_req.prompt.text.contains("what does this do?"));
    }

    #[test]
    fn adapt_response_to_capability() {
        let response = InferenceResponse {
            text: "test output".into(),
        };
        let capability_resp = adapt_response(response).unwrap();
        match capability_resp {
            contract::CapabilityResponse::BuildSummary(resp) => {
                assert!(resp.success);
                assert_eq!(resp.summary, "test output");
            }
            _ => panic!("unexpected response variant"),
        }
    }
}
