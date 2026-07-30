use std::path::PathBuf;

use crate::model_manager::error::ModelManagerError;

pub fn models_dir() -> Result<PathBuf, ModelManagerError> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| ModelManagerError::Storage("cannot determine home directory".into()))?;

    let dir = PathBuf::from(home).join(".cotrex").join("models");

    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }

    Ok(dir)
}

pub fn model_file_path(filename: &str) -> Result<PathBuf, ModelManagerError> {
    Ok(models_dir()?.join(filename))
}

pub fn is_installed(filename: &str) -> Result<bool, ModelManagerError> {
    let path = model_file_path(filename)?;
    Ok(path.exists())
}

pub fn list_installed() -> Result<Vec<String>, ModelManagerError> {
    let dir = models_dir()?;
    let mut files = Vec::new();

    if dir.exists() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "gguf")
                && let Some(name) = path.file_stem()
            {
                files.push(name.to_string_lossy().into_owned());
            }
        }
    }

    files.sort();
    Ok(files)
}

pub fn remove(filename: &str) -> Result<(), ModelManagerError> {
    let path = model_file_path(filename)?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_dir_creates_directory() {
        let dir = models_dir().unwrap();
        assert!(dir.exists());
        let lossy = dir.to_string_lossy();
        assert!(
            lossy.contains(".cotrex"),
            "path should contain .cotrex: {lossy}"
        );
        assert!(
            lossy.ends_with("models"),
            "path should end with models: {lossy}"
        );
    }

    #[test]
    fn model_file_path_correct() {
        let path = model_file_path("test.gguf").unwrap();
        assert!(path.to_string_lossy().contains("test.gguf"));
    }

    #[test]
    fn is_installed_false_for_missing() {
        assert!(!is_installed("nonexistent-model.gguf").unwrap());
    }

    #[test]
    fn list_installed_returns_vec() {
        let list = list_installed().unwrap();
        assert!(list.is_empty() || list.iter().all(|s| !s.is_empty()));
    }
}
