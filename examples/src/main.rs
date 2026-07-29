use contract::{BuildSummaryRequest, ExplainRustRequest, RequestMetadata};
use mock::MockProvider;
use runtime::CapabilityProviderExt;

fn main() {
    let provider = MockProvider;

    // Build summary: successful build
    let resp = provider
        .build_summary(BuildSummaryRequest {
            metadata: RequestMetadata::new(),
            command: "cargo build --release".into(),
            exit_code: 0,
            stdout: "Finished release profile [optimized]".into(),
            stderr: String::new(),
            prompt: "Summarize build: cargo build --release".into(),
            temperature: 0.1,
            max_tokens: 512,
        })
        .unwrap();
    println!("=== Build Summary (success) ===");
    println!("Success: {}", resp.success);
    println!("Summary: {}", resp.summary);
    println!("Recommendation: {:?}", resp.recommendation);
    println!();

    // Build summary: failed build
    let resp = provider
        .build_summary(BuildSummaryRequest {
            metadata: RequestMetadata::new(),
            command: "cargo build".into(),
            exit_code: 101,
            stdout: String::new(),
            stderr: "error[E0308]: mismatched types\n --> src/main.rs:5:12".into(),
            prompt: "Summarize build: cargo build".into(),
            temperature: 0.1,
            max_tokens: 512,
        })
        .unwrap();
    println!("=== Build Summary (failure) ===");
    println!("Success: {}", resp.success);
    println!("Summary: {}", resp.summary);
    println!("Recommendation: {:?}", resp.recommendation);
    println!();

    // Explain Rust
    let resp = provider
        .explain_rust(ExplainRustRequest {
            metadata: RequestMetadata::new(),
            source: r#"
fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}
"#
            .into(),
            question: "What does this function do?".into(),
            prompt: "Explain: What does this function do?\nfn fibonacci(n: u32) -> u32 { ... }"
                .into(),
            temperature: 0.2,
            max_tokens: 1024,
        })
        .unwrap();
    println!("=== Explain Rust ===");
    println!("{}", resp.explanation);
}
