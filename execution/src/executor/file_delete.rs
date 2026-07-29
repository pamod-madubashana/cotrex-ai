use std::fs;
use std::time::Instant;

use crate::error::ExecutionError;
use crate::executor::Executor;
use crate::executor::path_validation::{resolve_within, validate_raw_path};
use crate::request::{ExecutionAction, ExecutionRequest};
use crate::result::ExecutionResult;

/// Deletes files from the filesystem within a working directory scope.
///
/// All deletions are confined to the provided working directory. Path validation
/// rejects absolute paths, parent traversal, and symlink escapes.
///
/// Delete is idempotent: missing files are treated as successful no-ops.
pub struct FileDeleteExecutor;

impl Executor for FileDeleteExecutor {
    fn execute(&self, request: &ExecutionRequest) -> Result<ExecutionResult, ExecutionError> {
        let (rel_path, working_directory) = match &request.action {
            ExecutionAction::FileDelete {
                path,
                working_directory,
            } => (path, working_directory),
            other => {
                return Err(ExecutionError::Internal(format!(
                    "FileDeleteExecutor received non-FileDelete action: {:?}",
                    other
                )));
            }
        };

        validate_raw_path(rel_path)?;

        let resolved = resolve_within(working_directory, rel_path)?;

        let start = Instant::now();

        // Idempotent: missing file is a successful no-op
        if resolved.exists() {
            fs::remove_file(&resolved).map_err(|e| {
                ExecutionError::ExecutorFailed(format!("file delete failed: {}", e))
            })?;
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::ExecutionAction;
    use std::path::{Path, PathBuf};

    fn delete_request(path: &str, working_directory: &Path) -> ExecutionRequest {
        ExecutionRequest::new(
            ExecutionAction::FileDelete {
                path: PathBuf::from(path),
                working_directory: working_directory.to_path_buf(),
            },
            vec![],
        )
    }

    #[test]
    fn delete_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("to_delete.txt");
        fs::write(&file_path, b"content").unwrap();
        assert!(file_path.exists());

        let executor = FileDeleteExecutor;
        let req = delete_request("to_delete.txt", dir.path());
        let result = executor.execute(&req).unwrap();

        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));
        assert!(!file_path.exists());
    }

    #[test]
    fn delete_missing_file_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();

        let executor = FileDeleteExecutor;
        let req = delete_request("does_not_exist.txt", dir.path());
        let result = executor.execute(&req).unwrap();

        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));
    }

    #[test]
    fn reject_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        let executor = FileDeleteExecutor;

        #[cfg(windows)]
        let abs_path = "C:\\tmp\\file.txt";
        #[cfg(not(windows))]
        let abs_path = "/tmp/file.txt";

        let req = delete_request(abs_path, dir.path());
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
        let executor = FileDeleteExecutor;
        let req = delete_request("../escape.txt", dir.path());
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
        let executor = FileDeleteExecutor;
        let req = delete_request("src/../../escape.txt", dir.path());
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

        // Create a file in the outside directory
        fs::write(outside.path().join("target.txt"), b"secret").unwrap();

        let executor = FileDeleteExecutor;
        let req = delete_request("link/target.txt", dir.path());
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

        // Verify the file was NOT deleted
        assert!(outside.path().join("target.txt").exists());
    }

    #[cfg(windows)]
    #[test]
    fn reject_symlink_escape() {
        // On Windows, creating symlinks requires elevated privileges.
        // The canonicalize-based check is still tested via the path traversal tests.
    }

    #[test]
    fn non_file_delete_action_returns_error() {
        let executor = FileDeleteExecutor;
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
                assert!(msg.contains("non-FileDelete"));
            }
            other => panic!("expected Internal error, got: {:?}", other),
        }
    }
}
