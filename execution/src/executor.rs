pub mod command;

use crate::error::ExecutionError;
use crate::request::ExecutionRequest;
use crate::result::ExecutionResult;

// ---------------------------------------------------------------------------
// Executor Trait (RFC-0004)
// ---------------------------------------------------------------------------

/// Performs an action in the external world.
///
/// The engine dispatches execution through this trait. Each executor
/// handles a specific domain (commands, file writes, file deletes).
///
/// Executors are `Send + Sync` — the engine may hold them across threads.
/// Implementations must not rely on single-threaded access.
///
/// # Responsibility
///
/// - Perform the action described by the request
/// - Return outcome metadata as [`ExecutionResult`]
/// - Return [`ExecutionError::ExecutorFailed`] on failure
///
/// # Non-responsibility
///
/// - Assigning event IDs or sequence numbers
/// - Updating projections
/// - Interpreting output contents
/// - Deciding what to execute next
pub trait Executor: Send + Sync {
    /// Execute the request and return the result.
    fn execute(&self, request: &ExecutionRequest) -> Result<ExecutionResult, ExecutionError>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::ExecutionAction;
    use std::path::PathBuf;

    /// A minimal test executor that always succeeds.
    struct SucceedExecutor;

    impl Executor for SucceedExecutor {
        fn execute(&self, request: &ExecutionRequest) -> Result<ExecutionResult, ExecutionError> {
            Ok(ExecutionResult {
                execution_id: request.id,
                success: true,
                exit_code: Some(0),
                duration_ms: 0,
                error: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
    }

    #[test]
    fn succeed_executor_returns_ok() {
        let executor = SucceedExecutor;
        let req = ExecutionRequest::new(
            ExecutionAction::CommandRun {
                command: "echo hello".into(),
                working_directory: PathBuf::from("."),
            },
            vec![],
        );
        let result = executor.execute(&req).unwrap();
        assert!(result.success);
        assert_eq!(result.execution_id, req.id);
    }
}
