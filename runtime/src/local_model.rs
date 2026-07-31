use crate::ProviderError;
use crate::config::ResolvedConfig;

// ---------------------------------------------------------------------------
// Chat message
//
// A role/content pair for structured chat prompts. Provider-agnostic:
// the provider converts these to whatever template format the model expects.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

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
    /// Raw prompt text (fallback when messages is empty).
    pub prompt: Prompt,
    /// Structured chat messages. If non-empty, the provider should apply
    /// the model's chat template (e.g. ChatML) instead of using raw text.
    pub messages: Vec<ChatMessage>,
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
            messages: vec![],
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
    fn local_model_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Box<dyn LocalModel>>();
    }
}
