use std::fmt;

// ---------------------------------------------------------------------------
// Execution Error (RFC-0004)
// ---------------------------------------------------------------------------

/// Errors that can occur during execution.
///
/// Covers the full execution lifecycle: validation failures, event store
/// errors, executor failures, and internal engine errors.
#[derive(Debug)]
pub enum ExecutionError {
    /// The capability validator rejected the request.
    ValidationFailed(String),

    /// The EventStore rejected an append.
    EventAppendFailed(kernel::EventStoreError),

    /// The executor failed to perform the action.
    ExecutorFailed(String),

    /// An internal engine error occurred (e.g., lock poisoning).
    Internal(String),
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ValidationFailed(reason) => write!(f, "validation failed: {}", reason),
            Self::EventAppendFailed(e) => write!(f, "event append failed: {}", e),
            Self::ExecutorFailed(reason) => write!(f, "executor failed: {}", reason),
            Self::Internal(reason) => write!(f, "internal error: {}", reason),
        }
    }
}

impl std::error::Error for ExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EventAppendFailed(e) => Some(e),
            _ => None,
        }
    }
}

impl From<kernel::EventStoreError> for ExecutionError {
    fn from(e: kernel::EventStoreError) -> Self {
        Self::EventAppendFailed(e)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_failed_display() {
        let e = ExecutionError::ValidationFailed("missing capability".into());
        assert_eq!(e.to_string(), "validation failed: missing capability");
    }

    #[test]
    fn executor_failed_display() {
        let e = ExecutionError::ExecutorFailed("process crashed".into());
        assert_eq!(e.to_string(), "executor failed: process crashed");
    }

    #[test]
    fn internal_display() {
        let e = ExecutionError::Internal("lock poisoned".into());
        assert_eq!(e.to_string(), "internal error: lock poisoned");
    }

    #[test]
    fn event_append_is_error() {
        let e = ExecutionError::EventAppendFailed(kernel::EventStoreError::Backpressure);
        assert!(std::error::Error::source(&e).is_some());
    }
}
