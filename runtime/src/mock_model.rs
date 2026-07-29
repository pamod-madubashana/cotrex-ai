use crate::{
    InferenceRequest, InferenceResponse, LocalModel, ModelInfo, ProviderError, ResolvedConfig,
};

// ---------------------------------------------------------------------------
// MockLocalModel
//
// Deterministic mock model for testing. Returns canned responses without
// any inference.
// ---------------------------------------------------------------------------

pub struct MockLocalModel {
    info: ModelInfo,
}

impl MockLocalModel {
    pub fn new() -> Self {
        Self {
            info: ModelInfo {
                name: "mock-model".into(),
                version: "0.1.0".into(),
                backend: "mock".into(),
            },
        }
    }

    pub fn with_info(info: ModelInfo) -> Self {
        Self { info }
    }
}

impl Default for MockLocalModel {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalModel for MockLocalModel {
    fn load(&mut self, _config: &ResolvedConfig) -> Result<(), ProviderError> {
        Ok(())
    }

    fn infer(&self, request: InferenceRequest) -> Result<InferenceResponse, ProviderError> {
        Ok(InferenceResponse {
            text: format!("mock: {}", request.prompt.text),
        })
    }

    fn unload(&mut self) -> Result<(), ProviderError> {
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

    #[test]
    fn mock_model_load_succeeds() {
        let mut model = MockLocalModel::new();
        let config = ResolvedConfig::default();
        assert!(model.load(&config).is_ok());
    }

    #[test]
    fn mock_model_infer_returns_canned_response() {
        let model = MockLocalModel::new();
        let req = InferenceRequest {
            prompt: crate::Prompt::new("test prompt"),
            temperature: 0.1,
            max_tokens: 100,
        };
        let resp = model.infer(req).unwrap();
        assert_eq!(resp.text, "mock: test prompt");
    }

    #[test]
    fn mock_model_unload_succeeds() {
        let mut model = MockLocalModel::new();
        assert!(model.unload().is_ok());
    }

    #[test]
    fn mock_model_info_returns_metadata() {
        let model = MockLocalModel::new();
        let info = model.info();
        assert_eq!(info.name, "mock-model");
        assert_eq!(info.version, "0.1.0");
        assert_eq!(info.backend, "mock");
    }

    #[test]
    fn mock_model_with_custom_info() {
        let info = ModelInfo {
            name: "custom".into(),
            version: "1.0.0".into(),
            backend: "custom-backend".into(),
        };
        let model = MockLocalModel::with_info(info);
        let info = model.info();
        assert_eq!(info.name, "custom");
    }

    #[test]
    fn mock_model_default() {
        let model = MockLocalModel::default();
        let info = model.info();
        assert_eq!(info.name, "mock-model");
    }
}
