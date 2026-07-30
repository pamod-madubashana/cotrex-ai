use serde::{Deserialize, Serialize};

use crate::model_manager::error::ModelManagerError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDefinition {
    pub id: String,
    pub filename: String,
    pub url: String,
    pub sha256: Option<String>,
    pub size: u64,
    pub context: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelRegistry {
    pub models: Vec<ModelDefinition>,
}

impl ModelRegistry {
    pub fn parse(content: &str) -> Result<Self, ModelManagerError> {
        toml::from_str(content)
            .map_err(|e| ModelManagerError::Registry(format!("failed to parse registry: {e}")))
    }

    pub fn find(&self, id: &str) -> Option<&ModelDefinition> {
        self.models.iter().find(|m| m.id == id)
    }

    pub fn built_in() -> Self {
        let content = include_str!("registry.toml");
        Self::parse(content).expect("default registry is always valid")
    }

    pub fn merge(&mut self, other: ModelRegistry) {
        for model in other.models {
            self.models.retain(|m| m.id != model.id);
            self.models.push(model);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_toml() {
        let content = r#"
[[models]]
id = "test-model"
filename = "test.gguf"
url = "https://example.com/test.gguf"
size = 1000
"#;
        let registry = ModelRegistry::parse(content).unwrap();
        assert_eq!(registry.models.len(), 1);
        assert_eq!(registry.models[0].id, "test-model");
    }

    #[test]
    fn find_existing_model() {
        let registry = ModelRegistry::built_in();
        let model = registry.find("qwen2.5-0.5b");
        assert!(model.is_some());
        assert_eq!(model.unwrap().filename, "qwen2.5-0.5b-instruct-q4_k_m.gguf");
    }

    #[test]
    fn find_missing_model() {
        let registry = ModelRegistry::built_in();
        assert!(registry.find("nonexistent").is_none());
    }

    #[test]
    fn merge_user_overrides_default() {
        let mut registry = ModelRegistry::built_in();
        let user = ModelRegistry::parse(
            r#"
[[models]]
id = "qwen2.5-0.5b"
filename = "custom-qwen.gguf"
url = "https://example.com/custom.gguf"
size = 999
"#,
        )
        .unwrap();
        registry.merge(user);
        let model = registry.find("qwen2.5-0.5b").unwrap();
        assert_eq!(model.filename, "custom-qwen.gguf");
        assert_eq!(model.size, 999);
    }
}
