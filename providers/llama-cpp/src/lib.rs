use runtime::{
    InferenceRequest, InferenceResponse, LocalModel, ModelInfo, ProviderError, ResolvedConfig,
};
use std::path::PathBuf;

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
// LlamaCppModel
//
// First real LocalModel implementation. Stateless per-inference: each
// infer() call creates a fresh session, executes, and destroys it.
// ---------------------------------------------------------------------------

pub struct LlamaCppModel {
    model_path: Option<PathBuf>,
    info: ModelInfo,
    loaded_config: Option<LoadedConfig>,
}

impl LlamaCppModel {
    pub fn new() -> Self {
        Self {
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

        self.loaded_config = Some(LoadedConfig {
            model_path: path.clone(),
            context: config.context,
            threads: config.threads,
            gpu_layers: config.gpu_layers,
        });

        self.model_path = Some(path);
        self.info = ModelInfo {
            name: config.model_name.clone(),
            version: "loaded".into(),
            backend: "llama.cpp".into(),
        };

        Ok(())
    }

    fn infer(&self, request: InferenceRequest) -> Result<InferenceResponse, ProviderError> {
        if self.model_path.is_none() {
            return Err(ProviderError::Model("model not loaded".into()));
        }

        let _loaded = self
            .loaded_config
            .as_ref()
            .ok_or_else(|| ProviderError::Model("model not loaded".into()))?;

        // Stateless inference: create session, execute, destroy.
        // Actual llama.cpp integration will replace this block.
        let output = format!("llama.cpp: {}", request.prompt.text);

        Ok(InferenceResponse { text: output })
    }

    fn unload(&mut self) -> Result<(), ProviderError> {
        self.model_path = None;
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
        assert!(model.model_path.is_none());
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
        assert!(model.model_path.is_some());
        model.unload().unwrap();
        assert!(model.model_path.is_none());

        // Second cycle
        model.load(&config).unwrap();
        assert!(model.model_path.is_some());
        model.unload().unwrap();
        assert!(model.model_path.is_none());
    }

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

    #[test]
    fn load_transitions_to_ready() {
        let model = LlamaCppModel::new();
        let (config, _dir) = fake_model_config();
        let mut provider = LocalProvider::new(model, config, test_info());
        provider.load().unwrap();
        assert_eq!(provider.state(), contract::ProviderState::Ready);
    }

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

    #[test]
    fn health_reflects_state() {
        let model = LlamaCppModel::new();
        let (config, _dir) = fake_model_config();
        let mut provider = LocalProvider::new(model, config, test_info());

        // Uninitialized → Degraded
        assert!(matches!(
            provider.health(),
            contract::ProviderHealth::Degraded { .. }
        ));

        // Ready → Healthy
        provider.load().unwrap();
        assert!(matches!(
            provider.health(),
            contract::ProviderHealth::Healthy
        ));

        // Unloaded → Degraded
        provider.unload().unwrap();
        assert!(matches!(
            provider.health(),
            contract::ProviderHealth::Degraded { .. }
        ));
    }
}
