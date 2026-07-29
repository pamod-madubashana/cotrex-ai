use std::path::PathBuf;
use std::time::SystemTime;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Execution events (RFC-0004, Section 5)
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
    pub working_directory: PathBuf,
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
// File operations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileOperation {
    Created,
    Modified,
    Deleted,
}

// ---------------------------------------------------------------------------
// FileChanged event
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChanged {
    pub path: PathBuf,
    pub operation: FileOperation,
    pub timestamp: SystemTime,
}

// ---------------------------------------------------------------------------
// Event payload
// ---------------------------------------------------------------------------

/// An immutable event recorded in the EventStore.
///
/// Each variant represents a specific type of state change. The EventStore
/// is append-only; events are never modified after creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventPayload {
    /// A file was created, modified, or deleted.
    FileChanged(FileChanged),
    /// An execution request was accepted by the engine.
    ExecutionRequested(ExecutionRequested),
    /// An execution completed successfully.
    ExecutionCompleted(ExecutionCompleted),
    /// An execution failed.
    ExecutionFailed(ExecutionFailed),
}

// ---------------------------------------------------------------------------
// Event envelope
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub id: Uuid,
    pub sequence: u64,
    pub occurred_at: SystemTime,
    pub payload: EventPayload,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_operation_equality() {
        assert_eq!(FileOperation::Created, FileOperation::Created);
        assert_ne!(FileOperation::Created, FileOperation::Modified);
    }

    #[test]
    fn event_payload_clone() {
        let payload = EventPayload::FileChanged(FileChanged {
            path: PathBuf::from("test.txt"),
            operation: FileOperation::Created,
            timestamp: SystemTime::now(),
        });
        let _cloned = payload.clone();
    }

    #[test]
    fn event_is_clone() {
        let event = Event {
            id: Uuid::new_v4(),
            sequence: 1,
            occurred_at: SystemTime::now(),
            payload: EventPayload::FileChanged(FileChanged {
                path: PathBuf::from("test.txt"),
                operation: FileOperation::Created,
                timestamp: SystemTime::now(),
            }),
        };
        let _cloned = event.clone();
    }

    // -----------------------------------------------------------------------
    // Execution event construction tests
    // -----------------------------------------------------------------------

    #[test]
    fn execution_requested_construction() {
        let event = ExecutionRequested {
            execution_id: Uuid::new_v4(),
            command: "cargo build".into(),
            working_directory: PathBuf::from("/project"),
            requested_at: SystemTime::now(),
        };
        assert_eq!(event.command, "cargo build");
        assert_eq!(event.working_directory, PathBuf::from("/project"));
    }

    #[test]
    fn execution_completed_construction() {
        let event = ExecutionCompleted {
            execution_id: Uuid::new_v4(),
            exit_code: 0,
            duration_ms: 42,
            completed_at: SystemTime::now(),
        };
        assert_eq!(event.exit_code, 0);
        assert_eq!(event.duration_ms, 42);
    }

    #[test]
    fn execution_failed_construction() {
        let event = ExecutionFailed {
            execution_id: Uuid::new_v4(),
            error: "command not found".into(),
            duration_ms: 10,
            failed_at: SystemTime::now(),
        };
        assert_eq!(event.error, "command not found");
        assert_eq!(event.duration_ms, 10);
    }

    // -----------------------------------------------------------------------
    // EventPayload wrapping tests
    // -----------------------------------------------------------------------

    #[test]
    fn event_payload_wraps_execution_requested() {
        let req = ExecutionRequested {
            execution_id: Uuid::new_v4(),
            command: "ls".into(),
            working_directory: PathBuf::from("."),
            requested_at: SystemTime::now(),
        };
        let payload = EventPayload::ExecutionRequested(req.clone());
        match &payload {
            EventPayload::ExecutionRequested(inner) => assert_eq!(*inner, req),
            _ => panic!("expected ExecutionRequested variant"),
        }
    }

    #[test]
    fn event_payload_wraps_execution_completed() {
        let comp = ExecutionCompleted {
            execution_id: Uuid::new_v4(),
            exit_code: 0,
            duration_ms: 100,
            completed_at: SystemTime::now(),
        };
        let payload = EventPayload::ExecutionCompleted(comp.clone());
        match &payload {
            EventPayload::ExecutionCompleted(inner) => assert_eq!(*inner, comp),
            _ => panic!("expected ExecutionCompleted variant"),
        }
    }

    #[test]
    fn event_payload_wraps_execution_failed() {
        let fail = ExecutionFailed {
            execution_id: Uuid::new_v4(),
            error: "timeout".into(),
            duration_ms: 5000,
            failed_at: SystemTime::now(),
        };
        let payload = EventPayload::ExecutionFailed(fail.clone());
        match &payload {
            EventPayload::ExecutionFailed(inner) => assert_eq!(*inner, fail),
            _ => panic!("expected ExecutionFailed variant"),
        }
    }

    // -----------------------------------------------------------------------
    // Execution event behavior tests (clone, equality, debug)
    // -----------------------------------------------------------------------

    #[test]
    fn execution_requested_is_clone() {
        let event = ExecutionRequested {
            execution_id: Uuid::new_v4(),
            command: "cargo test".into(),
            working_directory: PathBuf::from("/project"),
            requested_at: SystemTime::now(),
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn execution_completed_is_clone() {
        let event = ExecutionCompleted {
            execution_id: Uuid::new_v4(),
            exit_code: 1,
            duration_ms: 200,
            completed_at: SystemTime::now(),
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn execution_failed_is_clone() {
        let event = ExecutionFailed {
            execution_id: Uuid::new_v4(),
            error: "segfault".into(),
            duration_ms: 30,
            failed_at: SystemTime::now(),
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn execution_requested_equality() {
        let id = Uuid::new_v4();
        let ts = SystemTime::now();
        let a = ExecutionRequested {
            execution_id: id,
            command: "ls".into(),
            working_directory: PathBuf::from("."),
            requested_at: ts,
        };
        let b = ExecutionRequested {
            execution_id: id,
            command: "ls".into(),
            working_directory: PathBuf::from("."),
            requested_at: ts,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn execution_completed_equality() {
        let id = Uuid::new_v4();
        let ts = SystemTime::now();
        let a = ExecutionCompleted {
            execution_id: id,
            exit_code: 0,
            duration_ms: 42,
            completed_at: ts,
        };
        let b = ExecutionCompleted {
            execution_id: id,
            exit_code: 0,
            duration_ms: 42,
            completed_at: ts,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn execution_failed_equality() {
        let id = Uuid::new_v4();
        let ts = SystemTime::now();
        let a = ExecutionFailed {
            execution_id: id,
            error: "fail".into(),
            duration_ms: 10,
            failed_at: ts,
        };
        let b = ExecutionFailed {
            execution_id: id,
            error: "fail".into(),
            duration_ms: 10,
            failed_at: ts,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn execution_events_debug_format() {
        let req = ExecutionRequested {
            execution_id: Uuid::new_v4(),
            command: "echo".into(),
            working_directory: PathBuf::from("/tmp"),
            requested_at: SystemTime::now(),
        };
        let debug = format!("{:?}", req);
        assert!(debug.contains("ExecutionRequested"));

        let comp = ExecutionCompleted {
            execution_id: Uuid::new_v4(),
            exit_code: 0,
            duration_ms: 1,
            completed_at: SystemTime::now(),
        };
        let debug = format!("{:?}", comp);
        assert!(debug.contains("ExecutionCompleted"));

        let fail = ExecutionFailed {
            execution_id: Uuid::new_v4(),
            error: "err".into(),
            duration_ms: 1,
            failed_at: SystemTime::now(),
        };
        let debug = format!("{:?}", fail);
        assert!(debug.contains("ExecutionFailed"));
    }

    #[test]
    fn event_payload_equality_across_variants() {
        let id = Uuid::new_v4();
        let ts = SystemTime::now();

        let payload_a = EventPayload::ExecutionRequested(ExecutionRequested {
            execution_id: id,
            command: "ls".into(),
            working_directory: PathBuf::from("."),
            requested_at: ts,
        });
        let payload_b = EventPayload::ExecutionRequested(ExecutionRequested {
            execution_id: id,
            command: "ls".into(),
            working_directory: PathBuf::from("."),
            requested_at: ts,
        });
        assert_eq!(payload_a, payload_b);

        // Different variants are not equal
        let payload_c = EventPayload::ExecutionCompleted(ExecutionCompleted {
            execution_id: id,
            exit_code: 0,
            duration_ms: 0,
            completed_at: ts,
        });
        assert_ne!(payload_a, payload_c);
    }
}
