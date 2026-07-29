use std::path::PathBuf;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Capability (RFC-0004, Section 6)
// ---------------------------------------------------------------------------

/// A declared ability an execution request requires.
///
/// The capability set is closed. Adding a new capability is a protocol
/// revision per RFC-0004.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Execute shell commands.
    CommandRun,
    /// Create or modify files.
    FileWrite,
    /// Remove files.
    FileDelete,
}

// ---------------------------------------------------------------------------
// Execution Action
// ---------------------------------------------------------------------------

/// A typed description of an action to perform in the external world.
///
/// Each variant maps to a specific execution domain. Using a typed enum
/// instead of opaque command strings allows the engine to dispatch to
/// the correct executor and validate capabilities precisely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionAction {
    /// Run a shell command in a working directory.
    CommandRun {
        /// The command string to execute.
        command: String,
        /// The working directory for execution.
        working_directory: PathBuf,
    },
    /// Write content to a file.
    FileWrite {
        /// The target file path (relative to working directory).
        path: PathBuf,
        /// The content to write.
        content: Vec<u8>,
        /// The working directory for file operations.
        working_directory: PathBuf,
    },
    /// Delete a file.
    FileDelete {
        /// The file path to remove (relative to working directory).
        path: PathBuf,
        /// The working directory for file operations.
        working_directory: PathBuf,
    },
}

// ---------------------------------------------------------------------------
// Execution Request
// ---------------------------------------------------------------------------

/// A typed description of an execution request.
///
/// Wraps an [`ExecutionAction`] with the capabilities required to perform it.
/// The engine validates capabilities before dispatching to an executor.
#[derive(Debug, Clone)]
pub struct ExecutionRequest {
    /// Unique identifier for this execution request.
    pub id: Uuid,
    /// The action to perform.
    pub action: ExecutionAction,
    /// Capabilities required to perform this action.
    pub required_capabilities: Vec<Capability>,
}

impl ExecutionRequest {
    /// Create a new execution request with a generated UUID.
    pub fn new(action: ExecutionAction, required_capabilities: Vec<Capability>) -> Self {
        Self {
            id: Uuid::new_v4(),
            action,
            required_capabilities,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_equality() {
        assert_eq!(Capability::CommandRun, Capability::CommandRun);
        assert_ne!(Capability::CommandRun, Capability::FileWrite);
    }

    #[test]
    fn action_is_clone() {
        let action = ExecutionAction::CommandRun {
            command: "cargo build".into(),
            working_directory: PathBuf::from("/project"),
        };
        let _cloned = action.clone();
    }

    #[test]
    fn request_new_generates_id() {
        let action = ExecutionAction::FileDelete {
            path: PathBuf::from("/tmp/file.txt"),
            working_directory: PathBuf::from("."),
        };
        let req = ExecutionRequest::new(action, vec![Capability::FileDelete]);
        assert_eq!(req.required_capabilities, vec![Capability::FileDelete]);
    }

    #[test]
    fn request_is_clone() {
        let req = ExecutionRequest::new(
            ExecutionAction::CommandRun {
                command: "ls".into(),
                working_directory: PathBuf::from("."),
            },
            vec![Capability::CommandRun],
        );
        let _cloned = req.clone();
    }
}
