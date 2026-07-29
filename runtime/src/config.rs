use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Configuration errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse config file: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("config error: {0}")]
    Invalid(String),
}

// ---------------------------------------------------------------------------
// Global config
//
// Located at ~/.config/cotrex/config.toml
// Contains machine-level defaults.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalConfig {
    pub provider: Option<String>,

    #[serde(default)]
    pub engine: Option<EngineConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EngineConfig {
    pub threads: Option<u32>,
    pub gpu_layers: Option<u32>,
}

// ---------------------------------------------------------------------------
// Project config
//
// Located at cotrex.toml (project root).
// Contains repository-specific behavior.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectConfig {
    #[serde(default)]
    pub model: Option<ModelConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelConfig {
    pub name: Option<String>,
    pub path: Option<PathBuf>,
    pub context: Option<u32>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

// ---------------------------------------------------------------------------
// Resolved config
//
// The final configuration after merging global, project, and defaults.
// This is what providers receive.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub backend: String,
    pub model_name: String,
    pub model_path: PathBuf,
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
            model_path: PathBuf::from("models/mock.gguf"),
            context: 4096,
            temperature: 0.1,
            max_tokens: 512,
            threads: 4,
            gpu_layers: 0,
        }
    }
}

impl ResolvedConfig {
    /// Load and merge configuration from global and project paths.
    ///
    /// Merge order: compiled defaults → global config → project config.
    pub fn load(
        global_path: Option<&Path>,
        project_path: Option<&Path>,
    ) -> Result<Self, ConfigError> {
        let global = load_global(global_path)?;
        let project = load_project(project_path)?;

        Ok(Self::merge(global, project))
    }

    fn merge(global: GlobalConfig, project: ProjectConfig) -> Self {
        let engine = global.engine.unwrap_or_default();
        let model = project.model.unwrap_or_default();

        Self {
            backend: global.provider.unwrap_or_else(|| "mock".into()),
            model_name: model.name.unwrap_or_else(|| "unknown".into()),
            model_path: model
                .path
                .unwrap_or_else(|| PathBuf::from("models/unknown.gguf")),
            context: model.context.unwrap_or(4096),
            temperature: model.temperature.unwrap_or(0.1),
            max_tokens: model.max_tokens.unwrap_or(512),
            threads: engine.threads.unwrap_or(4),
            gpu_layers: engine.gpu_layers.unwrap_or(0),
        }
    }
}

// ---------------------------------------------------------------------------
// Loaders
// ---------------------------------------------------------------------------

fn load_global(path: Option<&Path>) -> Result<GlobalConfig, ConfigError> {
    let path = match path {
        Some(p) => p.to_path_buf(),
        None => {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_default();
            PathBuf::from(home)
                .join(".config")
                .join("cotrex")
                .join("config.toml")
        }
    };

    if !path.exists() {
        return Ok(GlobalConfig::default());
    }

    let content = std::fs::read_to_string(&path)?;
    let config: GlobalConfig = toml::from_str(&content)?;
    Ok(config)
}

fn load_project(path: Option<&Path>) -> Result<ProjectConfig, ConfigError> {
    let path = match path {
        Some(p) => p.to_path_buf(),
        None => PathBuf::from("cotrex.toml"),
    };

    if !path.exists() {
        return Ok(ProjectConfig::default());
    }

    let content = std::fs::read_to_string(&path)?;
    let config: ProjectConfig = toml::from_str(&content)?;
    Ok(config)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_config() {
        let config = ResolvedConfig::default();
        assert_eq!(config.backend, "mock");
        assert_eq!(config.model_name, "mock-model");
        assert_eq!(config.context, 4096);
        assert_eq!(config.temperature, 0.1);
        assert_eq!(config.max_tokens, 512);
        assert_eq!(config.threads, 4);
        assert_eq!(config.gpu_layers, 0);
    }

    #[test]
    fn merge_empty_configs_uses_defaults() {
        let config = ResolvedConfig::merge(GlobalConfig::default(), ProjectConfig::default());
        assert_eq!(config.backend, "mock");
        assert_eq!(config.context, 4096);
    }

    #[test]
    fn merge_global_config() {
        let global = GlobalConfig {
            provider: Some("llama.cpp".into()),
            engine: Some(EngineConfig {
                threads: Some(8),
                gpu_layers: Some(32),
            }),
        };
        let config = ResolvedConfig::merge(global, ProjectConfig::default());
        assert_eq!(config.backend, "llama.cpp");
        assert_eq!(config.threads, 8);
        assert_eq!(config.gpu_layers, 32);
        assert_eq!(config.context, 4096);
    }

    #[test]
    fn merge_project_config() {
        let project = ProjectConfig {
            model: Some(ModelConfig {
                name: Some("qwen3".into()),
                path: Some(PathBuf::from("/models/qwen3.gguf")),
                context: Some(8192),
                temperature: Some(0.2),
                max_tokens: Some(1024),
            }),
        };
        let config = ResolvedConfig::merge(GlobalConfig::default(), project);
        assert_eq!(config.model_name, "qwen3");
        assert_eq!(config.model_path, PathBuf::from("/models/qwen3.gguf"));
        assert_eq!(config.context, 8192);
        assert_eq!(config.temperature, 0.2);
        assert_eq!(config.max_tokens, 1024);
    }

    #[test]
    fn project_overrides_global() {
        let global = GlobalConfig {
            provider: Some("llama.cpp".into()),
            engine: Some(EngineConfig {
                threads: Some(8),
                gpu_layers: Some(32),
            }),
        };
        let project = ProjectConfig {
            model: Some(ModelConfig {
                name: Some("custom".into()),
                path: Some(PathBuf::from("/custom.gguf")),
                context: Some(16384),
                temperature: Some(0.5),
                max_tokens: Some(2048),
            }),
        };
        let config = ResolvedConfig::merge(global, project);
        assert_eq!(config.model_name, "custom");
        assert_eq!(config.model_path, PathBuf::from("/custom.gguf"));
        assert_eq!(config.context, 16384);
        assert_eq!(config.temperature, 0.5);
        assert_eq!(config.max_tokens, 2048);
        assert_eq!(config.threads, 8);
        assert_eq!(config.gpu_layers, 32);
    }

    #[test]
    fn load_global_from_file() {
        let dir = tempdir().unwrap();
        let global_path = dir.path().join("config.toml");

        std::fs::write(
            &global_path,
            r#"
provider = "llama.cpp"

[engine]
threads = 16
gpu_layers = 40
"#,
        )
        .unwrap();

        let config = ResolvedConfig::load(Some(&global_path), None).unwrap();
        assert_eq!(config.backend, "llama.cpp");
        assert_eq!(config.threads, 16);
        assert_eq!(config.gpu_layers, 40);
    }

    #[test]
    fn load_project_from_file() {
        let dir = tempdir().unwrap();
        let project_path = dir.path().join("cotrex.toml");

        std::fs::write(
            &project_path,
            r#"
[model]
name = "qwen3"
path = "/models/qwen3.gguf"
context = 8192
temperature = 0.2
"#,
        )
        .unwrap();

        let config = ResolvedConfig::load(None, Some(&project_path)).unwrap();
        assert_eq!(config.model_name, "qwen3");
        assert_eq!(config.model_path, PathBuf::from("/models/qwen3.gguf"));
        assert_eq!(config.context, 8192);
        assert_eq!(config.temperature, 0.2);
    }

    #[test]
    fn missing_files_use_defaults() {
        let config = ResolvedConfig::load(
            Some(Path::new("/nonexistent/global.toml")),
            Some(Path::new("/nonexistent/cotrex.toml")),
        )
        .unwrap();
        assert_eq!(config.backend, "mock");
        assert_eq!(config.context, 4096);
    }

    #[test]
    fn invalid_toml_returns_error() {
        let dir = tempdir().unwrap();
        let global_path = dir.path().join("config.toml");

        std::fs::write(&global_path, "=== not valid toml {{{").unwrap();

        let result = ResolvedConfig::load(Some(&global_path), None);
        assert!(result.is_err());
    }
}
