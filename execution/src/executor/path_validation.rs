use std::fs;
use std::path::{Path, PathBuf};

use crate::error::ExecutionError;

/// Validate the raw input path before joining with working directory.
///
/// Rejects absolute paths and parent traversal components.
pub fn validate_raw_path(path: &Path) -> Result<(), ExecutionError> {
    if path.is_absolute() {
        return Err(ExecutionError::ExecutorFailed(format!(
            "path must be relative, got absolute: {}",
            path.display()
        )));
    }

    for component in path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(ExecutionError::ExecutorFailed(format!(
                "path must not contain '..' components: {}",
                path.display()
            )));
        }
    }

    Ok(())
}

/// Resolve the final path and verify it stays within working directory.
///
/// Protects against symlink escapes and unexpected filesystem resolution.
/// Handles non-existent targets by canonicalizing the nearest existing ancestor.
pub fn resolve_within(
    working_directory: &Path,
    rel_path: &Path,
) -> Result<PathBuf, ExecutionError> {
    let joined = working_directory.join(rel_path);

    let canonical_wd = fs::canonicalize(working_directory).map_err(|e| {
        ExecutionError::ExecutorFailed(format!("failed to canonicalize working directory: {}", e))
    })?;

    // Try to canonicalize the target; if it doesn't exist yet, canonicalize the parent
    let canonical_target = if joined.exists() {
        fs::canonicalize(&joined).map_err(|e| {
            ExecutionError::ExecutorFailed(format!("failed to canonicalize target path: {}", e))
        })?
    } else {
        // Walk up to find an existing ancestor, then join remaining components
        let mut ancestor = joined.as_path();
        let mut remaining = Vec::new();

        loop {
            match fs::canonicalize(ancestor) {
                Ok(canonical) => {
                    // Rebuild path: canonical ancestor + remaining components
                    let mut result = canonical;
                    for comp in remaining.iter().rev() {
                        result = result.join(comp);
                    }
                    break result;
                }
                Err(_) => {
                    // Strip last component and try parent
                    match ancestor.parent() {
                        Some(parent) => {
                            remaining.push(ancestor.strip_prefix(parent).unwrap().to_path_buf());
                            ancestor = parent;
                        }
                        None => {
                            return Err(ExecutionError::ExecutorFailed(
                                "failed to resolve path: no canonical ancestor found".into(),
                            ));
                        }
                    }
                }
            }
        }
    };

    if !canonical_target.starts_with(&canonical_wd) {
        return Err(ExecutionError::ExecutorFailed(format!(
            "path escapes working directory: {}",
            rel_path.display()
        )));
    }

    Ok(joined)
}
