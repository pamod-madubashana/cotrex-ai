use runtime::{
    InferenceRequest, InferenceResponse, LocalModel, ModelInfo, ProviderError, ResolvedConfig,
};
use std::path::PathBuf;

#[cfg(feature = "real-inference")]
use llama_cpp_2::context::params::LlamaContextParams;
#[cfg(feature = "real-inference")]
use llama_cpp_2::llama_backend::LlamaBackend;
#[cfg(feature = "real-inference")]
use llama_cpp_2::llama_batch::LlamaBatch;
#[cfg(feature = "real-inference")]
use llama_cpp_2::model::params::LlamaModelParams;
#[cfg(feature = "real-inference")]
use llama_cpp_2::model::{AddBos, LlamaModel};
#[cfg(feature = "real-inference")]
use llama_cpp_2::sampling::LlamaSampler;
#[cfg(feature = "real-inference")]
use std::num::NonZeroU32;
#[cfg(feature = "real-inference")]
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// LoadedConfig
//
// Immutable subset of ResolvedConfig that this backend actually uses.
// Keeps the provider depending only on what it needs.
// ---------------------------------------------------------------------------

pub struct LoadedConfig {
    pub model_path: PathBuf,
    pub context: u32,
    pub threads: u32,
    pub gpu_layers: u32,
}

// ---------------------------------------------------------------------------
// Global llama.cpp backend — initialized once, shared across all instances.
// LlamaBackend is a ZST wrapper around a global FFI singleton. OnceLock
// ensures thread-safe one-time initialization without ownership issues.
// ---------------------------------------------------------------------------

#[cfg(feature = "real-inference")]
static LLAMA_BACKEND: OnceLock<LlamaBackend> = OnceLock::new();

#[cfg(feature = "real-inference")]
fn get_backend() -> Result<&'static LlamaBackend, ProviderError> {
    Ok(LLAMA_BACKEND
        .get_or_init(|| LlamaBackend::init().expect("failed to initialize llama.cpp backend")))
}

// ---------------------------------------------------------------------------
// LlamaCppModel
//
// First real LocalModel implementation. Stateless per-inference: each
// infer() call creates a fresh context, executes, and destroys it.
// ---------------------------------------------------------------------------

pub struct LlamaCppModel {
    #[cfg(feature = "real-inference")]
    model: Option<LlamaModel>,
    #[cfg(not(feature = "real-inference"))]
    model_path: Option<PathBuf>,
    info: ModelInfo,
    loaded_config: Option<LoadedConfig>,
}

impl LlamaCppModel {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "real-inference")]
            model: None,
            #[cfg(not(feature = "real-inference"))]
            model_path: None,
            info: ModelInfo {
                name: "llama.cpp".into(),
                version: "unknown".into(),
                backend: "llama.cpp".into(),
            },
            loaded_config: None,
        }
    }
}

impl Default for LlamaCppModel {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalModel for LlamaCppModel {
    fn load(&mut self, config: &ResolvedConfig) -> Result<(), ProviderError> {
        let path = config.model_path.clone();

        if !path.exists() {
            return Err(ProviderError::Model(format!(
                "model file not found: {}",
                path.display()
            )));
        }

        #[cfg(feature = "real-inference")]
        {
            let backend = get_backend()?;

            let model_params = LlamaModelParams::default().with_n_gpu_layers(config.gpu_layers);

            let model = LlamaModel::load_from_file(backend, &path, &model_params)
                .map_err(|e| ProviderError::Model(format!("model load failed: {e}")))?;

            self.model = Some(model);
        }

        #[cfg(not(feature = "real-inference"))]
        {
            self.model_path = Some(path.clone());
        }

        self.loaded_config = Some(LoadedConfig {
            model_path: path,
            context: config.context,
            threads: config.threads,
            gpu_layers: config.gpu_layers,
        });

        self.info = ModelInfo {
            name: config.model_name.clone(),
            version: "loaded".into(),
            backend: "llama.cpp".into(),
        };

        Ok(())
    }

    fn infer(&self, request: InferenceRequest) -> Result<InferenceResponse, ProviderError> {
        #[cfg(feature = "real-inference")]
        {
            let backend = get_backend()?;
            let model = self
                .model
                .as_ref()
                .ok_or_else(|| ProviderError::Model("model not loaded".into()))?;
            let loaded = self
                .loaded_config
                .as_ref()
                .ok_or_else(|| ProviderError::Model("model not loaded".into()))?;

            // Tokenize prompt
            let tokens = model
                .str_to_token(&request.prompt.text, AddBos::Always)
                .map_err(|e| ProviderError::Model(format!("tokenization failed: {e}")))?;

            // Create context
            let ctx_params = LlamaContextParams::default()
                .with_n_ctx(NonZeroU32::new(loaded.context))
                .with_n_threads(loaded.threads as i32);
            let mut ctx = model
                .new_context(backend, ctx_params)
                .map_err(|e| ProviderError::Model(format!("context creation failed: {e}")))?;

            // Process prompt tokens
            if tokens.is_empty() {
                return Err(ProviderError::Model(
                    "tokenization produced no tokens".into(),
                ));
            }
            let mut batch = LlamaBatch::new(tokens.len() + request.max_tokens as usize + 1, 1);
            let last_index = (tokens.len() - 1) as i32;
            for (i, token) in (0_i32..).zip(tokens) {
                batch
                    .add(token, i, &[0], i == last_index)
                    .map_err(|e| ProviderError::Model(format!("batch add failed: {e}")))?;
            }
            ctx.decode(&mut batch)
                .map_err(|e| ProviderError::Model(format!("decode failed: {e}")))?;

            // Generate tokens
            let mut sampler =
                LlamaSampler::chain_simple([LlamaSampler::dist(1234), LlamaSampler::greedy()]);

            let mut output = String::new();
            let mut n_cur = batch.n_tokens();
            let mut decoder = encoding_rs::UTF_8.new_decoder();

            #[allow(clippy::explicit_counter_loop)]
            for _ in 0..request.max_tokens {
                let token = sampler.sample(&ctx, batch.n_tokens() - 1);
                sampler.accept(token);

                if model.is_eog_token(token) {
                    break;
                }

                let text = model
                    .token_to_piece(token, &mut decoder, true, None)
                    .map_err(|e| ProviderError::Model(format!("token decode failed: {e}")))?;
                output.push_str(&text);

                batch.clear();
                batch
                    .add(token, n_cur, &[0], true)
                    .map_err(|e| ProviderError::Model(format!("batch add failed: {e}")))?;
                ctx.decode(&mut batch)
                    .map_err(|e| ProviderError::Model(format!("decode failed: {e}")))?;

                n_cur += 1;
            }

            Ok(InferenceResponse { text: output })
        }

        #[cfg(not(feature = "real-inference"))]
        {
            if self.model_path.is_none() {
                return Err(ProviderError::Model("model not loaded".into()));
            }
            let _loaded = self
                .loaded_config
                .as_ref()
                .ok_or_else(|| ProviderError::Model("model not loaded".into()))?;
            let output = format!("llama.cpp: {}", request.prompt.text);
            Ok(InferenceResponse { text: output })
        }
    }

    fn unload(&mut self) -> Result<(), ProviderError> {
        #[cfg(feature = "real-inference")]
        {
            self.model = None;
        }

        #[cfg(not(feature = "real-inference"))]
        {
            self.model_path = None;
        }

        self.loaded_config = None;
        self.info = ModelInfo {
            name: "llama.cpp".into(),
            version: "unknown".into(),
            backend: "llama.cpp".into(),
        };
        Ok(())
    }

    fn info(&self) -> ModelInfo {
        self.info.clone()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use runtime::{CapabilityProvider, LocalProvider};

    fn default_config() -> ResolvedConfig {
        ResolvedConfig::default()
    }

    #[test]
    fn new_constructs_with_defaults() {
        let model = LlamaCppModel::new();
        assert_eq!(model.info.name, "llama.cpp");
        assert_eq!(model.info.version, "unknown");
        assert_eq!(model.info.backend, "llama.cpp");
        #[cfg(not(feature = "real-inference"))]
        assert!(model.model_path.is_none());
        #[cfg(feature = "real-inference")]
        assert!(model.model.is_none());
        assert!(model.loaded_config.is_none());
    }

    #[test]
    fn info_before_load_returns_defaults() {
        let model = LlamaCppModel::new();
        let info = model.info();
        assert_eq!(info.backend, "llama.cpp");
        assert_eq!(info.version, "unknown");
    }

    #[test]
    fn load_fails_nonexistent_path() {
        let mut model = LlamaCppModel::new();
        let config = ResolvedConfig {
            model_path: PathBuf::from("/nonexistent/model.gguf"),
            ..default_config()
        };
        let result = model.load(&config);
        assert!(result.is_err());
        match result {
            Err(ProviderError::Model(msg)) => {
                assert!(msg.contains("model file not found"));
            }
            _ => panic!("expected ProviderError::Model"),
        }
    }

    #[test]
    fn infer_before_load_returns_error() {
        let model = LlamaCppModel::new();
        let request = InferenceRequest {
            prompt: runtime::Prompt::new("test"),
            temperature: 0.1,
            max_tokens: 100,
        };
        let result = model.infer(request);
        assert!(result.is_err());
    }

    #[cfg(not(feature = "real-inference"))]
    #[test]
    fn infer_after_unload_returns_error() {
        let mut model = LlamaCppModel::new();
        let config = default_config();

        // Create a temporary file to simulate a model
        let dir = tempfile::tempdir().unwrap();
        let model_path = dir.path().join("test.gguf");
        std::fs::write(&model_path, b"fake model").unwrap();

        let config = ResolvedConfig {
            model_path,
            ..config
        };

        model.load(&config).unwrap();
        model.unload().unwrap();

        let request = InferenceRequest {
            prompt: runtime::Prompt::new("test"),
            temperature: 0.1,
            max_tokens: 100,
        };
        let result = model.infer(request);
        assert!(result.is_err());
    }

    #[cfg(not(feature = "real-inference"))]
    #[test]
    fn load_unload_load_unload_cycle() {
        let mut model = LlamaCppModel::new();
        let config = default_config();

        let dir = tempfile::tempdir().unwrap();
        let model_path = dir.path().join("test.gguf");
        std::fs::write(&model_path, b"fake model").unwrap();

        let config = ResolvedConfig {
            model_path,
            ..config
        };

        // First cycle
        model.load(&config).unwrap();
        #[cfg(not(feature = "real-inference"))]
        assert!(model.model_path.is_some());
        #[cfg(feature = "real-inference")]
        assert!(model.model.is_some());
        model.unload().unwrap();
        #[cfg(not(feature = "real-inference"))]
        assert!(model.model_path.is_none());
        #[cfg(feature = "real-inference")]
        assert!(model.model.is_none());

        // Second cycle
        model.load(&config).unwrap();
        #[cfg(not(feature = "real-inference"))]
        assert!(model.model_path.is_some());
        #[cfg(feature = "real-inference")]
        assert!(model.model.is_some());
        model.unload().unwrap();
        #[cfg(not(feature = "real-inference"))]
        assert!(model.model_path.is_none());
        #[cfg(feature = "real-inference")]
        assert!(model.model.is_none());
    }

    #[cfg(not(feature = "real-inference"))]
    #[test]
    fn load_populates_info() {
        let mut model = LlamaCppModel::new();
        let dir = tempfile::tempdir().unwrap();
        let model_path = dir.path().join("test.gguf");
        std::fs::write(&model_path, b"fake model").unwrap();

        let config = ResolvedConfig {
            model_path,
            model_name: "qwen3".into(),
            ..default_config()
        };

        model.load(&config).unwrap();
        let info = model.info();
        assert_eq!(info.name, "qwen3");
        assert_eq!(info.version, "loaded");
        assert_eq!(info.backend, "llama.cpp");
    }

    #[test]
    fn llama_cpp_model_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LlamaCppModel>();
    }

    // =========================================================================
    // Lifecycle tests via LocalProvider<LlamaCppModel>
    // =========================================================================

    fn test_info() -> contract::ProviderInfo {
        contract::ProviderInfo {
            name: "llama.cpp".into(),
            version: "0.1.0".into(),
            supported_capabilities: vec![
                contract::CapabilityKind::BuildSummary,
                contract::CapabilityKind::ExplainRust,
            ],
        }
    }

    fn fake_model_config() -> (ResolvedConfig, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let model_path = dir.path().join("test.gguf");
        std::fs::write(&model_path, b"fake model").unwrap();
        (
            ResolvedConfig {
                model_path,
                ..ResolvedConfig::default()
            },
            dir,
        )
    }

    #[test]
    fn provider_starts_uninitialized() {
        let model = LlamaCppModel::new();
        let (config, _dir) = fake_model_config();
        let provider = LocalProvider::new(model, config, test_info());
        assert_eq!(provider.state(), contract::ProviderState::Uninitialized);
    }

    #[cfg(not(feature = "real-inference"))]
    #[test]
    fn load_transitions_to_ready() {
        let model = LlamaCppModel::new();
        let (config, _dir) = fake_model_config();
        let mut provider = LocalProvider::new(model, config, test_info());
        provider.load().unwrap();
        assert_eq!(provider.state(), contract::ProviderState::Ready);
    }

    #[cfg(not(feature = "real-inference"))]
    #[test]
    fn unload_transitions_to_uninitialized() {
        let model = LlamaCppModel::new();
        let (config, _dir) = fake_model_config();
        let mut provider = LocalProvider::new(model, config, test_info());
        provider.load().unwrap();
        provider.unload().unwrap();
        assert_eq!(provider.state(), contract::ProviderState::Uninitialized);
    }

    #[test]
    fn failed_load_transitions_to_failed() {
        let model = LlamaCppModel::new();
        let config = ResolvedConfig {
            model_path: PathBuf::from("/nonexistent/model.gguf"),
            ..ResolvedConfig::default()
        };
        let mut provider = LocalProvider::new(model, config, test_info());
        let result = provider.load();
        assert!(result.is_err());
        assert_eq!(provider.state(), contract::ProviderState::Failed);
    }

    #[cfg(not(feature = "real-inference"))]
    #[test]
    fn failed_can_retry_to_loading() {
        let model = LlamaCppModel::new();
        let config = ResolvedConfig {
            model_path: PathBuf::from("/nonexistent/model.gguf"),
            ..ResolvedConfig::default()
        };
        let mut provider = LocalProvider::new(model, config, test_info());

        // First load fails
        assert!(provider.load().is_err());
        assert_eq!(provider.state(), contract::ProviderState::Failed);

        // Switch to valid config and retry
        let valid_model = LlamaCppModel::new();
        let (valid_config, _dir) = fake_model_config();
        let mut provider = LocalProvider::new(valid_model, valid_config, test_info());
        assert!(provider.load().is_ok());
        assert_eq!(provider.state(), contract::ProviderState::Ready);
    }

    #[cfg(not(feature = "real-inference"))]
    #[test]
    fn health_reflects_state() {
        let model = LlamaCppModel::new();
        let (config, _dir) = fake_model_config();
        let mut provider = LocalProvider::new(model, config, test_info());

        // Uninitialized -> Degraded
        assert!(matches!(
            provider.health(),
            contract::ProviderHealth::Degraded { .. }
        ));

        // Ready -> Healthy
        provider.load().unwrap();
        assert!(matches!(
            provider.health(),
            contract::ProviderHealth::Healthy
        ));

        // Unloaded -> Degraded
        provider.unload().unwrap();
        assert!(matches!(
            provider.health(),
            contract::ProviderHealth::Degraded { .. }
        ));
    }
}

// ---------------------------------------------------------------------------
// Integration tests (require real-inference feature + GGUF model)
// ---------------------------------------------------------------------------

#[cfg(all(test, feature = "real-inference"))]
mod integration {
    use super::*;
    use runtime::{CapabilityProvider, LocalProvider};
    use std::path::PathBuf;

    fn test_model_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("qwen2.5-0.5b-instruct-q4_k_m.gguf")
    }

    #[test]
    fn load_populates_info_from_gguf() {
        let mut model = LlamaCppModel::new();
        let config = ResolvedConfig {
            model_path: test_model_path(),
            model_name: "qwen2.5-0.5b".into(),
            ..ResolvedConfig::default()
        };
        model.load(&config).unwrap();
        let info = model.info();
        assert_eq!(info.backend, "llama.cpp");
        assert_eq!(info.version, "loaded");
    }

    #[test]
    fn infer_returns_generated_text() {
        let mut model = LlamaCppModel::new();
        let config = ResolvedConfig {
            model_path: test_model_path(),
            ..ResolvedConfig::default()
        };
        model.load(&config).unwrap();

        let request = InferenceRequest {
            prompt: runtime::Prompt::new("What is 2 + 2? Answer with just the number."),
            temperature: 0.0,
            max_tokens: 16,
        };
        let response = model.infer(request).unwrap();
        assert!(!response.text.is_empty());
        assert_ne!(
            response.text,
            "llama.cpp: What is 2 + 2? Answer with just the number."
        );
    }

    #[test]
    fn build_summary_end_to_end() {
        let mut model = LlamaCppModel::new();
        let config = ResolvedConfig {
            model_path: test_model_path(),
            ..ResolvedConfig::default()
        };
        model.load(&config).unwrap();

        let info = contract::ProviderInfo {
            name: "llama.cpp".into(),
            version: "0.1.0".into(),
            supported_capabilities: vec![
                contract::CapabilityKind::BuildSummary,
                contract::CapabilityKind::ExplainRust,
            ],
        };
        let mut provider = LocalProvider::new(model, config, info);
        provider.load().unwrap();

        let request = contract::CapabilityRequest::BuildSummary(contract::BuildSummaryRequest {
            metadata: contract::RequestMetadata::new(),
            command: "cargo build".into(),
            exit_code: 0,
            stdout: "Compiling project v0.1.0\nFinished dev [optimized] target(s)".into(),
            stderr: String::new(),
            prompt: "Summarize the build output".into(),
            temperature: 0.1,
            max_tokens: 256,
        });

        let response = provider.execute(request).unwrap();
        match response {
            contract::CapabilityResponse::BuildSummary(resp) => {
                assert!(!resp.summary.is_empty());
            }
            _ => panic!("expected BuildSummary response"),
        }
    }

    #[test]
    fn build_summary_failure_explains_error() {
        let mut model = LlamaCppModel::new();
        let config = ResolvedConfig {
            model_path: test_model_path(),
            ..ResolvedConfig::default()
        };
        model.load(&config).unwrap();

        let info = contract::ProviderInfo {
            name: "llama.cpp".into(),
            version: "0.1.0".into(),
            supported_capabilities: vec![
                contract::CapabilityKind::BuildSummary,
                contract::CapabilityKind::ExplainRust,
            ],
        };
        let mut provider = LocalProvider::new(model, config, info);
        provider.load().unwrap();

        let request = contract::CapabilityRequest::BuildSummary(contract::BuildSummaryRequest {
            metadata: contract::RequestMetadata::new(),
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
            contract::CapabilityResponse::BuildSummary(resp) => {
                assert!(!resp.summary.is_empty());
                assert!(resp.summary.len() > 20);
                let lower = resp.summary.to_lowercase();
                assert!(
                    lower.contains("error")
                        || lower.contains("fail")
                        || lower.contains("issue")
                        || lower.contains("problem"),
                    "summary should mention error/fail/issue/problem: {}",
                    resp.summary
                );
            }
            _ => panic!("expected BuildSummary response"),
        }
    }

    #[test]
    fn explain_rust_describes_code() {
        let mut model = LlamaCppModel::new();
        let config = ResolvedConfig {
            model_path: test_model_path(),
            ..ResolvedConfig::default()
        };
        model.load(&config).unwrap();

        let info = contract::ProviderInfo {
            name: "llama.cpp".into(),
            version: "0.1.0".into(),
            supported_capabilities: vec![
                contract::CapabilityKind::BuildSummary,
                contract::CapabilityKind::ExplainRust,
            ],
        };
        let mut provider = LocalProvider::new(model, config, info);
        provider.load().unwrap();

        let source = "fn main() {\n    let x = String::from(\"hello\");\n    let y = x;\n    println!(\"{}\", y);\n}";
        let request = contract::CapabilityRequest::ExplainRust(contract::ExplainRustRequest {
            metadata: contract::RequestMetadata::new(),
            source: source.into(),
            question: "What happens to x?".into(),
            prompt: format!("Explain this Rust code:\n{}", source),
            temperature: 0.1,
            max_tokens: 256,
        });

        let response = provider.execute(request).unwrap();
        match response {
            contract::CapabilityResponse::BuildSummary(resp) => {
                assert!(!resp.summary.is_empty());
                assert!(resp.summary.len() > 20);
                let lower = resp.summary.to_lowercase();
                assert!(
                    lower.contains("owner")
                        || lower.contains("move")
                        || lower.contains("borrow")
                        || lower.contains("clone")
                        || lower.contains("string"),
                    "explanation should mention ownership concepts: {}",
                    resp.summary
                );
            }
            _ => panic!("expected BuildSummary response (adapter wraps all responses)"),
        }
    }
}
