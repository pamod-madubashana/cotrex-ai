use std::time::SystemTime;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Execution Events (RFC-0004, Section 5)
// ---------------------------------------------------------------------------

/// Emitted when the Execution Engine accepts a request.
///
/// Records what was asked, not what happened. Appended to the EventStore
/// before the action is performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRequested {
    /// Unique identifier for this execution.
    pub execution_id: Uuid,
    /// The command that was requested.
    pub command: String,
    /// The working directory for execution.
    pub working_directory: std::path::PathBuf,
    /// Timestamp when the request was accepted.
    pub requested_at: SystemTime,
}

/// Emitted when execution finishes successfully.
///
/// Records outcome metadata. Does NOT store stdout/stderr content.
/// Large output belongs in external storage referenced by `execution_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionCompleted {
    /// Unique identifier for this execution.
    pub execution_id: Uuid,
    /// The process exit code.
    pub exit_code: i32,
    /// Duration of execution in milliseconds.
    pub duration_ms: u64,
    /// Timestamp when execution completed.
    pub completed_at: SystemTime,
}

/// Emitted when execution fails.
///
/// Records the error reason. Does NOT store full output dumps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionFailed {
    /// Unique identifier for this execution.
    pub execution_id: Uuid,
    /// Description of the failure.
    pub error: String,
    /// Duration of execution in milliseconds.
    pub duration_ms: u64,
    /// Timestamp when execution failed.
    pub failed_at: SystemTime,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requested_is_clone() {
        let event = ExecutionRequested {
            execution_id: Uuid::new_v4(),
            command: "cargo build".into(),
            working_directory: std::path::PathBuf::from("/project"),
            requested_at: SystemTime::now(),
        };
        let _cloned = event.clone();
    }

    #[test]
    fn completed_is_clone() {
        let event = ExecutionCompleted {
            execution_id: Uuid::new_v4(),
            exit_code: 0,
            duration_ms: 42,
            completed_at: SystemTime::now(),
        };
        let _cloned = event.clone();
    }

    #[test]
    fn failed_is_clone() {
        let event = ExecutionFailed {
            execution_id: Uuid::new_v4(),
            error: "command not found".into(),
            duration_ms: 10,
            failed_at: SystemTime::now(),
        };
        let _cloned = event.clone();
    }
}
