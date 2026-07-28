use contract::{
    BuildSummaryRequest, BuildSummaryResponse, CapabilityKind, CapabilityRequest,
    CapabilityResponse, ExplainRustRequest, ExplainRustResponse, ProviderHealth, ProviderInfo,
};
use runtime::{CapabilityProvider, RuntimeError};

/// Deterministic mock provider for protocol validation.
/// Returns realistic responses without any AI inference.
pub struct MockProvider;

impl CapabilityProvider for MockProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "mock".into(),
            version: "0.1.0".into(),
            supported_capabilities: vec![CapabilityKind::BuildSummary, CapabilityKind::ExplainRust],
        }
    }

    fn health(&self) -> ProviderHealth {
        ProviderHealth::Healthy
    }

    fn execute(&self, request: CapabilityRequest) -> Result<CapabilityResponse, RuntimeError> {
        match request {
            CapabilityRequest::BuildSummary(req) => Ok(CapabilityResponse::BuildSummary(
                build_summary_response(req),
            )),
            CapabilityRequest::ExplainRust(req) => {
                Ok(CapabilityResponse::ExplainRust(explain_rust_response(req)))
            }
        }
    }
}

fn build_summary_response(req: BuildSummaryRequest) -> BuildSummaryResponse {
    match req.exit_code {
        0 => BuildSummaryResponse {
            success: true,
            summary: "Build completed successfully.".into(),
            recommendation: None,
        },
        101 => BuildSummaryResponse {
            success: false,
            summary: "Compilation failed.".into(),
            recommendation: Some("Inspect compiler diagnostics.".into()),
        },
        102 => BuildSummaryResponse {
            success: false,
            summary: "Build failed: linker error.".into(),
            recommendation: Some("Verify all dependencies are linked correctly.".into()),
        },
        _ => BuildSummaryResponse {
            success: false,
            summary: format!("Build failed with exit code {}.", req.exit_code),
            recommendation: Some("Review command output for details.".into()),
        },
    }
}

fn explain_rust_response(req: ExplainRustRequest) -> ExplainRustResponse {
    let source_len = req.source.len();
    let has_fn = req.source.contains("fn ");
    let has_let = req.source.contains("let ");
    let has_impl = req.source.contains("impl ");

    let mut traits = Vec::new();
    if has_fn {
        traits.push("defines functions");
    }
    if has_let {
        traits.push("uses local variables");
    }
    if has_impl {
        traits.push("implements methods");
    }

    let detail = if traits.is_empty() {
        "no notable Rust patterns detected".into()
    } else {
        traits.join(", ")
    };

    ExplainRustResponse {
        explanation: format!(
            "The provided source ({} chars) {}. Question: {}",
            source_len, detail, req.question
        ),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use contract::RequestMetadata;
    use runtime::CapabilityProviderExt;

    fn metadata() -> RequestMetadata {
        RequestMetadata::new()
    }

    #[test]
    fn build_summary_success() {
        let provider = MockProvider;
        let resp = provider
            .build_summary(BuildSummaryRequest {
                metadata: metadata(),
                command: "cargo build".into(),
                exit_code: 0,
                stdout: "Finished dev profile".into(),
                stderr: String::new(),
            })
            .unwrap();
        assert!(resp.success);
        assert_eq!(resp.summary, "Build completed successfully.");
        assert!(resp.recommendation.is_none());
    }

    #[test]
    fn build_summary_compilation_failed() {
        let provider = MockProvider;
        let resp = provider
            .build_summary(BuildSummaryRequest {
                metadata: metadata(),
                command: "cargo build".into(),
                exit_code: 101,
                stdout: String::new(),
                stderr: "error[E0308]: mismatched types".into(),
            })
            .unwrap();
        assert!(!resp.success);
        assert_eq!(resp.summary, "Compilation failed.");
        assert_eq!(
            resp.recommendation.unwrap(),
            "Inspect compiler diagnostics."
        );
    }

    #[test]
    fn build_summary_linker_error() {
        let provider = MockProvider;
        let resp = provider
            .build_summary(BuildSummaryRequest {
                metadata: metadata(),
                command: "cargo build".into(),
                exit_code: 102,
                stdout: String::new(),
                stderr: "linker error".into(),
            })
            .unwrap();
        assert!(!resp.success);
        assert!(resp.summary.contains("linker error"));
    }

    #[test]
    fn build_summary_unknown_exit_code() {
        let provider = MockProvider;
        let resp = provider
            .build_summary(BuildSummaryRequest {
                metadata: metadata(),
                command: "cargo build".into(),
                exit_code: 99,
                stdout: String::new(),
                stderr: String::new(),
            })
            .unwrap();
        assert!(!resp.success);
        assert!(resp.summary.contains("exit code 99"));
    }

    #[test]
    fn explain_rust_with_function() {
        let provider = MockProvider;
        let resp = provider
            .explain_rust(ExplainRustRequest {
                metadata: metadata(),
                source: "fn main() {}".into(),
                question: "what does this do?".into(),
            })
            .unwrap();
        assert!(resp.explanation.contains("defines functions"));
        assert!(resp.explanation.contains("what does this do?"));
    }

    #[test]
    fn explain_rust_with_let_binding() {
        let provider = MockProvider;
        let resp = provider
            .explain_rust(ExplainRustRequest {
                metadata: metadata(),
                source: "let x = 42;".into(),
                question: "explain".into(),
            })
            .unwrap();
        assert!(resp.explanation.contains("uses local variables"));
    }

    #[test]
    fn explain_rust_with_impl_block() {
        let provider = MockProvider;
        let resp = provider
            .explain_rust(ExplainRustRequest {
                metadata: metadata(),
                source: "impl Foo { fn bar() {} }".into(),
                question: "what is this?".into(),
            })
            .unwrap();
        assert!(resp.explanation.contains("implements methods"));
    }

    #[test]
    fn explain_rust_with_multiple_patterns() {
        let provider = MockProvider;
        let resp = provider
            .explain_rust(ExplainRustRequest {
                metadata: metadata(),
                source: "fn foo() { let x = 1; }".into(),
                question: "explain".into(),
            })
            .unwrap();
        assert!(resp.explanation.contains("defines functions"));
        assert!(resp.explanation.contains("uses local variables"));
    }

    #[test]
    fn explain_rust_empty_source() {
        let provider = MockProvider;
        let resp = provider
            .explain_rust(ExplainRustRequest {
                metadata: metadata(),
                source: String::new(),
                question: "what?".into(),
            })
            .unwrap();
        assert!(resp.explanation.contains("no notable Rust patterns"));
    }

    #[test]
    fn execute_returns_matching_variant_for_build_summary() {
        let provider = MockProvider;
        let resp = provider
            .execute(CapabilityRequest::BuildSummary(BuildSummaryRequest {
                metadata: metadata(),
                command: "cargo test".into(),
                exit_code: 0,
                stdout: "ok".into(),
                stderr: String::new(),
            }))
            .unwrap();
        assert!(matches!(resp, CapabilityResponse::BuildSummary(_)));
    }

    #[test]
    fn execute_returns_matching_variant_for_explain_rust() {
        let provider = MockProvider;
        let resp = provider
            .execute(CapabilityRequest::ExplainRust(ExplainRustRequest {
                metadata: metadata(),
                source: String::new(),
                question: String::new(),
            }))
            .unwrap();
        assert!(matches!(resp, CapabilityResponse::ExplainRust(_)));
    }
}
