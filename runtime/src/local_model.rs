use crate::ProviderError;

// ---------------------------------------------------------------------------
// Prompt
//
// Wrapper for prompt text. Exists so it can evolve into structured context
// without breaking the trait.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub text: String,
}

impl Prompt {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

// ---------------------------------------------------------------------------
// Inference request/response
//
// Typed structures for inference. Parameters evolve without trait changes.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct InferenceRequest {
    pub prompt: Prompt,
    pub temperature: f32,
    pub max_tokens: u32,
}

#[derive(Debug, Clone)]
pub struct InferenceResponse {
    pub text: String,
}

// ---------------------------------------------------------------------------
// Model info
//
// Metadata about the model backend.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub version: String,
    pub backend: String,
}

// ---------------------------------------------------------------------------
// LocalModel trait
//
// Abstracts model loading, inference, and unloading. The model has no
// lifecycle awareness — the provider owns lifecycle management.
// ---------------------------------------------------------------------------

pub trait LocalModel: Send + Sync {
    /// Loads the model into memory.
    fn load(&mut self, config: &ResolvedConfig) -> Result<(), ProviderError>;

    /// Executes inference on the loaded model.
    fn infer(&self, request: InferenceRequest) -> Result<InferenceResponse, ProviderError>;

    /// Unloads the model from memory.
    fn unload(&mut self) -> Result<(), ProviderError>;

    /// Returns model metadata.
    fn info(&self) -> ModelInfo;
}

// ---------------------------------------------------------------------------
// ResolvedConfig (placeholder for Phase 4)
//
// For now, a minimal config that satisfies the trait. Will be expanded
// in Phase 4 (Configuration).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub backend: String,
    pub model_name: String,
    pub context: u32,
    pub temperature: f32,
    pub max_tokens: u32,
    pub threads: u32,
    pub gpu_layers: u32,
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            backend: "mock".into(),
            model_name: "mock-model".into(),
            context: 4096,
            temperature: 0.1,
            max_tokens: 512,
            threads: 4,
            gpu_layers: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt建造() {
        let prompt = Prompt::new("hello world");
        assert_eq!(prompt.text, "hello world");
    }

    #[test]
    fn prompt_from_string() {
        let prompt = Prompt::new(String::from("test"));
        assert_eq!(prompt.text, "test");
    }

    #[test]
    fn inference_request建造() {
        let req = InferenceRequest {
            prompt: Prompt::new("test"),
            temperature: 0.5,
            max_tokens: 100,
        };
        assert_eq!(req.prompt.text, "test");
        assert_eq!(req.temperature, 0.5);
        assert_eq!(req.max_tokens, 100);
    }

    #[test]
    fn inference_response建造() {
        let resp = InferenceResponse {
            text: "result".into(),
        };
        assert_eq!(resp.text, "result");
    }

    #[test]
    fn model_info建造() {
        let info = ModelInfo {
            name: "test".into(),
            version: "0.1.0".into(),
            backend: "mock".into(),
        };
        assert_eq!(info.name, "test");
    }

    #[test]
    fn resolved_config_default() {
        let config = ResolvedConfig::default();
        assert_eq!(config.backend, "mock");
        assert_eq!(config.context, 4096);
    }

    #[test]
    fn local_model_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Box<dyn LocalModel>>();
    }
}
