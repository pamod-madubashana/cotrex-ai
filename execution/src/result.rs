use uuid::Uuid;

// ---------------------------------------------------------------------------
// Execution Result (RFC-0004, Section 5)
// ---------------------------------------------------------------------------

/// The outcome of an execution attempt.
///
/// Contains transient runtime data including stdout and stderr.
/// Large output lives here only — it is never persisted in execution events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionResult {
    /// The ID of the execution that produced this result.
    pub execution_id: Uuid,
    /// Whether the execution completed successfully.
    pub success: bool,
    /// The exit code, if available (e.g., for command execution).
    pub exit_code: Option<i32>,
    /// Duration of execution in milliseconds.
    pub duration_ms: u64,
    /// Error message, if the execution failed.
    pub error: Option<String>,
    /// Captured stdout bytes.
    pub stdout: Vec<u8>,
    /// Captured stderr bytes.
    pub stderr: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_is_clone() {
        let result = ExecutionResult {
            execution_id: Uuid::new_v4(),
            success: true,
            exit_code: Some(0),
            duration_ms: 42,
            error: None,
            stdout: b"hello\n".to_vec(),
            stderr: Vec::new(),
        };
        let _cloned = result.clone();
    }

    #[test]
    fn result_success_fields() {
        let result = ExecutionResult {
            execution_id: Uuid::new_v4(),
            success: true,
            exit_code: Some(0),
            duration_ms: 100,
            error: None,
            stdout: b"output".to_vec(),
            stderr: Vec::new(),
        };
        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));
        assert!(result.error.is_none());
    }

    #[test]
    fn result_failure_fields() {
        let result = ExecutionResult {
            execution_id: Uuid::new_v4(),
            success: false,
            exit_code: Some(1),
            duration_ms: 50,
            error: Some("command not found".into()),
            stdout: Vec::new(),
            stderr: b"error output".to_vec(),
        };
        assert!(!result.success);
        assert_eq!(result.exit_code, Some(1));
        assert!(result.error.is_some());
    }
}
