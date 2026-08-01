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
    /// Performance tier: Fast, Balanced, Powerful, High-end, Enthusiast.
    #[serde(default)]
    pub tier: Option<String>,
    /// Minimum recommended RAM in GB.
    #[serde(default)]
    pub ram_gb: Option<u32>,
    /// Short one-line description of the model's strengths.
    #[serde(default)]
    pub description: Option<String>,
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
        let model = registry.find("gemma-3-1b");
        assert!(model.is_some());
        assert_eq!(model.unwrap().filename, "gemma-3-1b-it-q4_k_m.gguf");
    }

    #[test]
    fn model_has_tier_and_ram() {
        let registry = ModelRegistry::built_in();
        let model = registry.find("qwen3-8b").unwrap();
        assert_eq!(model.tier.as_deref(), Some("Powerful"));
        assert_eq!(model.ram_gb, Some(8));
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
id = "gemma-3-1b"
filename = "custom-gemma.gguf"
url = "https://example.com/custom.gguf"
size = 999
"#,
        )
        .unwrap();
        registry.merge(user);
        let model = registry.find("gemma-3-1b").unwrap();
        assert_eq!(model.filename, "custom-gemma.gguf");
        assert_eq!(model.size, 999);
    }
}
