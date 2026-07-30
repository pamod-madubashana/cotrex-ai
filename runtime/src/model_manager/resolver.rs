use std::path::PathBuf;

use crate::model_manager::error::ModelManagerError;
use crate::model_manager::registry::{ModelDefinition, ModelRegistry};
use crate::model_manager::storage;

pub struct ModelResolver {
    registry: ModelRegistry,
}

impl ModelResolver {
    pub fn new(registry: ModelRegistry) -> Self {
        Self { registry }
    }

    pub fn resolve(&self, id: &str) -> Result<PathBuf, ModelManagerError> {
        let model = self
            .registry
            .find(id)
            .ok_or_else(|| ModelManagerError::NotFound(id.into()))?;

        if !storage::is_installed(&model.filename)? {
            return Err(ModelManagerError::NotFound(format!(
                "model not installed: {id}. Run: cotrex model install {id}"
            )));
        }

        storage::model_file_path(&model.filename)
    }

    pub fn resolve_or_download(
        &self,
        id: &str,
        downloader: impl FnOnce(&ModelDefinition) -> Result<PathBuf, ModelManagerError>,
    ) -> Result<PathBuf, ModelManagerError> {
        match self.resolve(id) {
            Ok(path) => Ok(path),
            Err(ModelManagerError::NotFound(_)) => {
                let model = self
                    .registry
                    .find(id)
                    .ok_or_else(|| ModelManagerError::NotFound(id.into()))?;
                downloader(model)
            }
            Err(e) => Err(e),
        }
    }

    pub fn list(&self) -> Result<Vec<(ModelDefinition, bool)>, ModelManagerError> {
        let mut result = Vec::new();
        for model in &self.registry.models {
            let installed = storage::is_installed(&model.filename)?;
            result.push((model.clone(), installed));
        }
        Ok(result)
    }

    pub fn registry(&self) -> &ModelRegistry {
        &self.registry
    }
}

pub fn load_registry() -> Result<ModelRegistry, ModelManagerError> {
    let mut registry = ModelRegistry::built_in();

    let user_path = storage::models_dir()?.join("registry.toml");
    if user_path.exists() {
        let user_content = std::fs::read_to_string(&user_path)?;
        let user_registry = ModelRegistry::parse(&user_content)?;
        registry.merge(user_registry);
    }

    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_installed_model() {
        let registry = ModelRegistry::built_in();
        let resolver = ModelResolver::new(registry);
        // qwen2.5-0.5b is not installed in ~/.cotrex/models, so this should fail
        let result = resolver.resolve("qwen2.5-0.5b");
        assert!(result.is_err());
    }

    #[test]
    fn resolve_missing_model() {
        let registry = ModelRegistry::built_in();
        let resolver = ModelResolver::new(registry);
        let result = resolver.resolve("nonexistent");
        assert!(matches!(result, Err(ModelManagerError::NotFound(_))));
    }

    #[test]
    fn list_shows_all_models() {
        let registry = ModelRegistry::built_in();
        let resolver = ModelResolver::new(registry);
        let list = resolver.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].0.id, "qwen2.5-0.5b");
        // Not installed in test env
        assert!(!list[0].1);
    }

    #[test]
    fn load_registry_includes_defaults() {
        let registry = load_registry().unwrap();
        assert!(registry.find("qwen2.5-0.5b").is_some());
    }
}
