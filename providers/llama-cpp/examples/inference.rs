use llama_cpp_provider::LlamaCppModel;
use runtime::{InferenceRequest, LocalModel, Prompt, ResolvedConfig};
use std::path::PathBuf;

fn main() {
    let model_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures")
                .join("qwen2.5-0.5b-instruct-q4_k_m.gguf")
        });

    if !model_path.exists() {
        eprintln!("Model not found: {}", model_path.display());
        eprintln!("Usage: inference [path-to-gguf]");
        std::process::exit(1);
    }

    let config = ResolvedConfig {
        model_path,
        context: 4096,
        threads: 4,
        ..ResolvedConfig::default()
    };

    let mut model = LlamaCppModel::new();

    eprintln!("Loading model...");
    model.load(&config).expect("failed to load model");

    let info = model.info();
    eprintln!("Model loaded: {} ({})", info.name, info.version);

    let prompt = "Explain Rust ownership in 2 sentences.";
    eprintln!("\nPrompt: {}\n", prompt);

    let request = InferenceRequest {
        prompt: Prompt::new(prompt),
        messages: vec![],
        temperature: 0.1,
        max_tokens: 128,
    };

    eprintln!("Response:");
    match model.infer(request) {
        Ok(response) => println!("{}", response.text),
        Err(e) => {
            eprintln!("Inference failed: {e}");
            std::process::exit(1);
        }
    }

    model.unload().expect("failed to unload model");
    eprintln!("\nModel unloaded.");
}
