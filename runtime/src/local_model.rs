use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::ProviderError;
use crate::config::ResolvedConfig;

pub type TokenCallback = Arc<Mutex<dyn FnMut(&str) + Send + 'static>>;

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

#[derive(Clone)]
pub struct InferenceRequest {
    /// Raw prompt text (fallback when messages is empty).
    pub prompt: Prompt,
    /// Structured chat messages. If non-empty, the provider should apply
    /// the model's chat template (e.g. ChatML) instead of using raw text.
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
    pub max_tokens: u32,
    /// Optional token-by-token callback. When set, the provider sends each
    /// generated text piece through this callback before accumulating.
    /// Used by the UI layer for real-time streaming in User mode.
    pub token_callback: Option<TokenCallback>,
}

impl std::fmt::Debug for InferenceRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InferenceRequest")
            .field("prompt", &self.prompt)
            .field("messages", &self.messages)
            .field("temperature", &self.temperature)
            .field("max_tokens", &self.max_tokens)
            .field(
                "token_callback",
                &self.token_callback.as_ref().map(|_| "<callback>"),
            )
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct InferenceResponse {
    pub text: String,
    /// Optional per-phase profiling data. Populated by the provider when
    /// profiling is enabled; `None` in normal operation.
    pub profile: Option<InferProfile>,
}

/// Fine-grained timing for each phase of a single inference call.
/// The provider populates this during `infer()` so callers can see
/// exactly where time is spent.
#[derive(Debug, Clone, Default)]
pub struct InferProfile {
    pub chat_template: Duration,
    pub tokenize: Duration,
    pub new_context: Duration,
    pub prompt_decode: Duration,
    pub generation: Duration,
    pub total: Duration,
    pub prompt_tokens: usize,
    pub generated_tokens: usize,
}

impl InferProfile {
    /// Tokens per second for prompt processing.
    pub fn prompt_tok_s(&self) -> f64 {
        let secs = self.prompt_decode.as_secs_f64();
        if secs > 0.0 {
            self.prompt_tokens as f64 / secs
        } else {
            0.0
        }
    }

    /// Tokens per second for generation.
    pub fn gen_tok_s(&self) -> f64 {
        let secs = self.generation.as_secs_f64();
        if secs > 0.0 {
            self.generated_tokens as f64 / secs
        } else {
            0.0
        }
    }

    /// Print a human-readable breakdown to stderr.
    pub fn print(&self) {
        eprintln!("  ┌─ Inference Profile ─────────────────────────┐");
        eprintln!(
            "  │ chat_template  {:>8.1} ms",
            self.chat_template.as_secs_f64() * 1000.0
        );
        eprintln!(
            "  │ tokenize       {:>8.1} ms",
            self.tokenize.as_secs_f64() * 1000.0
        );
        eprintln!(
            "  │ new_context    {:>8.1} ms",
            self.new_context.as_secs_f64() * 1000.0
        );
        eprintln!(
            "  │ prompt_decode  {:>8.1} ms  ({} tok, {:.0} tok/s)",
            self.prompt_decode.as_secs_f64() * 1000.0,
            self.prompt_tokens,
            self.prompt_tok_s()
        );
        eprintln!(
            "  │ generation     {:>8.1} ms  ({} tok, {:.1} tok/s)",
            self.generation.as_secs_f64() * 1000.0,
            self.generated_tokens,
            self.gen_tok_s()
        );
        eprintln!(
            "  │ total          {:>8.1} ms",
            self.total.as_secs_f64() * 1000.0
        );
        eprintln!("  └────────────────────────────────────────────┘");
    }
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
            token_callback: None,
        };
        assert_eq!(req.prompt.text, "test");
        assert_eq!(req.temperature, 0.5);
        assert_eq!(req.max_tokens, 100);
    }

    #[test]
    fn inference_response建造() {
        let resp = InferenceResponse {
            text: "result".into(),
            profile: None,
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
