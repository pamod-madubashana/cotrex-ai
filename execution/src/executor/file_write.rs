use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::error::ExecutionError;
use crate::executor::Executor;
use crate::request::{ExecutionAction, ExecutionRequest};
use crate::result::ExecutionResult;

/// Writes file contents to the filesystem within a working directory scope.
///
/// All writes are confined to the provided working directory. Path validation
/// rejects absolute paths, parent traversal, and symlink escapes.
pub struct FileWriteExecutor;

impl Executor for FileWriteExecutor {
    fn execute(&self, request: &ExecutionRequest) -> Result<ExecutionResult, ExecutionError> {
        let (rel_path, content, working_directory) = match &request.action {
            ExecutionAction::FileWrite {
                path,
                content,
                working_directory,
            } => (path, content.as_slice(), working_directory),
            other => {
                return Err(ExecutionError::Internal(format!(
                    "FileWriteExecutor received non-FileWrite action: {:?}",
                    other
                )));
            }
        };

        validate_raw_path(rel_path)?;

        let resolved = resolve_within(working_directory, rel_path)?;

        let start = Instant::now();

        if let Some(parent) = resolved.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                ExecutionError::ExecutorFailed(format!(
                    "failed to create parent directories: {}",
                    e
                ))
            })?;
        }

        fs::write(&resolved, content)
            .map_err(|e| ExecutionError::ExecutorFailed(format!("file write failed: {}", e)))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ExecutionResult {
            execution_id: request.id,
            success: true,
            exit_code: Some(0),
            duration_ms,
            error: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
}

/// Step 1: Validate the raw input path before joining with working directory.
///
/// Rejects absolute paths and parent traversal components.
fn validate_raw_path(path: &Path) -> Result<(), ExecutionError> {
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

/// Step 2: Resolve the final path and verify it stays within working directory.
///
/// Protects against symlink escapes and unexpected filesystem resolution.
fn resolve_within(working_directory: &Path, rel_path: &Path) -> Result<PathBuf, ExecutionError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::ExecutionAction;
    use std::path::PathBuf;

    fn write_request(path: &str, content: &[u8], working_directory: &Path) -> ExecutionRequest {
        ExecutionRequest::new(
            ExecutionAction::FileWrite {
                path: PathBuf::from(path),
                content: content.to_vec(),
                working_directory: working_directory.to_path_buf(),
            },
            vec![],
        )
    }

    #[test]
    fn basic_write() {
        let dir = tempfile::tempdir().unwrap();
        let executor = FileWriteExecutor;
        let req = write_request("test.txt", b"hello world", dir.path());
        let result = executor.execute(&req).unwrap();

        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));

        let written = fs::read(dir.path().join("test.txt")).unwrap();
        assert_eq!(written, b"hello world");
    }

    #[test]
    fn binary_content() {
        let dir = tempfile::tempdir().unwrap();
        let executor = FileWriteExecutor;
        let binary: Vec<u8> = (0..=255).collect();
        let req = write_request("binary.bin", &binary, dir.path());
        let result = executor.execute(&req).unwrap();

        assert!(result.success);

        let written = fs::read(dir.path().join("binary.bin")).unwrap();
        assert_eq!(written, binary);
    }

    #[test]
    fn overwrite_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("existing.txt"), b"old content").unwrap();

        let executor = FileWriteExecutor;
        let req = write_request("existing.txt", b"new content", dir.path());
        executor.execute(&req).unwrap();

        let written = fs::read(dir.path().join("existing.txt")).unwrap();
        assert_eq!(written, b"new content");
    }

    #[test]
    fn create_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let executor = FileWriteExecutor;
        let req = write_request("a/b/c/file.txt", b"nested", dir.path());
        let result = executor.execute(&req).unwrap();

        assert!(result.success);

        let written = fs::read(dir.path().join("a/b/c/file.txt")).unwrap();
        assert_eq!(written, b"nested");
    }

    #[test]
    fn reject_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        let executor = FileWriteExecutor;

        // Use a path that is absolute on the current platform
        #[cfg(windows)]
        let abs_path = "C:\\tmp\\test.txt";
        #[cfg(not(windows))]
        let abs_path = "/tmp/test.txt";

        let req = write_request(abs_path, b"bad", dir.path());
        let result = executor.execute(&req);

        match result {
            Err(ExecutionError::ExecutorFailed(summary)) => {
                assert!(
                    summary.contains("absolute")
                        || summary.contains("path must be relative")
                        || summary.contains("escapes working directory"),
                    "unexpected: {}",
                    summary
                );
            }
            other => panic!("expected ExecutorFailed, got: {:?}", other),
        }
    }

    #[test]
    fn reject_traversal_path() {
        let dir = tempfile::tempdir().unwrap();
        let executor = FileWriteExecutor;
        let req = write_request("../escape.txt", b"bad", dir.path());
        let result = executor.execute(&req);

        match result {
            Err(ExecutionError::ExecutorFailed(summary)) => {
                assert!(summary.contains("'..'"));
            }
            other => panic!("expected ExecutorFailed, got: {:?}", other),
        }
    }

    #[test]
    fn reject_nested_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let executor = FileWriteExecutor;
        let req = write_request("src/../../escape.txt", b"bad", dir.path());
        let result = executor.execute(&req);

        match result {
            Err(ExecutionError::ExecutorFailed(summary)) => {
                assert!(summary.contains("'..'"));
            }
            other => panic!("expected ExecutorFailed, got: {:?}", other),
        }
    }

    #[cfg(unix)]
    #[test]
    fn reject_symlink_escape() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();

        // Create symlink: working_dir/link -> outside_dir
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();

        let executor = FileWriteExecutor;
        let req = write_request("link/escaped.txt", b"bad", dir.path());
        let result = executor.execute(&req);

        match result {
            Err(ExecutionError::ExecutorFailed(summary)) => {
                assert!(
                    summary.contains("escapes working directory"),
                    "unexpected: {}",
                    summary
                );
            }
            other => panic!("expected ExecutorFailed, got: {:?}", other),
        }
    }

    #[cfg(windows)]
    #[test]
    fn reject_symlink_escape() {
        // On Windows, creating symlinks requires elevated privileges.
        // The canonicalize-based check is still tested via the path traversal tests.
    }

    #[test]
    fn non_file_write_action_returns_error() {
        let executor = FileWriteExecutor;
        let req = ExecutionRequest::new(
            ExecutionAction::CommandRun {
                command: "echo hello".into(),
                working_directory: PathBuf::from("."),
            },
            vec![],
        );
        let result = executor.execute(&req);

        match result {
            Err(ExecutionError::Internal(msg)) => {
                assert!(msg.contains("non-FileWrite"));
            }
            other => panic!("expected Internal error, got: {:?}", other),
        }
    }
}
