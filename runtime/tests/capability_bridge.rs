use contract::*;
use runtime::{CapabilityProvider, LocalModel, LocalProvider, ResolvedConfig};
use std::path::PathBuf;

fn test_model_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../providers/llama-cpp/fixtures")
        .join("qwen2.5-0.5b-instruct-q4_k_m.gguf")
}

fn create_provider() -> LocalProvider<llama_cpp_provider::LlamaCppModel> {
    let mut model = llama_cpp_provider::LlamaCppModel::new();
    let config = ResolvedConfig {
        model_path: test_model_path(),
        ..ResolvedConfig::default()
    };
    model.load(&config).expect("failed to load model");

    let info = ProviderInfo {
        name: "llama.cpp".into(),
        version: "0.1.0".into(),
        supported_capabilities: vec![CapabilityKind::BuildSummary, CapabilityKind::ExplainRust],
    };
    let mut provider = LocalProvider::new(model, config, info);
    provider.load().expect("failed to init provider");
    provider
}

#[test]
fn build_summary_via_provider() {
    let provider = create_provider();

    let request = CapabilityRequest::BuildSummary(BuildSummaryRequest {
        metadata: RequestMetadata::new(),
        command: "cargo build".into(),
        exit_code: 1,
        stdout: String::new(),
        stderr: "error[E0599]: no method named `foo` found\n --> src/main.rs:5:10".into(),
        prompt: "Summarize the build failure".into(),
        temperature: 0.1,
        max_tokens: 256,
    });

    let response = provider.execute(request).unwrap();
    match response {
        CapabilityResponse::BuildSummary(resp) => {
            assert!(!resp.summary.is_empty());
            assert!(resp.summary.len() > 20);
        }
        _ => panic!("expected BuildSummary response"),
    }
}

#[test]
fn explain_rust_via_provider() {
    let provider = create_provider();

    let source = "fn main() {\n    let x = String::from(\"hello\");\n    let y = x;\n    println!(\"{}\", y);\n}";
    let request = CapabilityRequest::ExplainRust(ExplainRustRequest {
        metadata: RequestMetadata::new(),
        source: source.into(),
        question: "What happens to x?".into(),
        prompt: format!("Explain this Rust code:\n{}", source),
        temperature: 0.1,
        max_tokens: 256,
    });

    let response = provider.execute(request).unwrap();
    match response {
        CapabilityResponse::BuildSummary(resp) => {
            assert!(!resp.summary.is_empty());
            assert!(resp.summary.len() > 20);
        }
        _ => panic!("expected BuildSummary response (adapter wraps all)"),
    }
}
