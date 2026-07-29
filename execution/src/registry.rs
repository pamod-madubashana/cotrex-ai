use crate::executor::Executor;
use crate::request::ExecutionAction;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Executor Registry
// ---------------------------------------------------------------------------

/// Maps execution action types to their corresponding executors.
///
/// The registry is the extension point for adding new execution domains.
/// Each [`ExecutionAction`] variant maps to a [`Box<dyn Executor>`].
///
/// # Example
///
/// ```ignore
/// let mut registry = ExecutorRegistry::new();
/// registry.register(ExecutionActionDiscriminant::CommandRun, Box::new(cmd_executor));
/// ```
pub struct ExecutorRegistry {
    executors: HashMap<ExecutionActionDiscriminant, Box<dyn Executor>>,
}

/// Discriminant for matching action types to executors.
///
/// This mirrors [`ExecutionAction`] but without data, serving as a
/// lookup key in the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionActionDiscriminant {
    CommandRun,
    FileWrite,
    FileDelete,
}

impl From<&ExecutionAction> for ExecutionActionDiscriminant {
    fn from(action: &ExecutionAction) -> Self {
        match action {
            ExecutionAction::CommandRun { .. } => Self::CommandRun,
            ExecutionAction::FileWrite { .. } => Self::FileWrite,
            ExecutionAction::FileDelete { .. } => Self::FileDelete,
        }
    }
}

impl ExecutorRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            executors: HashMap::new(),
        }
    }

    /// Register an executor for a given action discriminant.
    ///
    /// If an executor was already registered for this discriminant, it is
    /// replaced.
    pub fn register(
        &mut self,
        discriminant: ExecutionActionDiscriminant,
        executor: Box<dyn Executor>,
    ) {
        self.executors.insert(discriminant, executor);
    }

    /// Look up the executor for a given action.
    ///
    /// Returns `None` if no executor is registered for the action type.
    pub fn get(&self, action: &ExecutionAction) -> Option<&dyn Executor> {
        let discriminant = ExecutionActionDiscriminant::from(action);
        self.executors.get(&discriminant).map(|e| e.as_ref())
    }

    /// Return the number of registered executors.
    pub fn len(&self) -> usize {
        self.executors.len()
    }

    /// Return true if no executors are registered.
    pub fn is_empty(&self) -> bool {
        self.executors.is_empty()
    }
}

impl Default for ExecutorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ExecutionError;
    use crate::request::ExecutionRequest;
    use crate::result::ExecutionResult;
    use std::path::PathBuf;

    struct DummyExecutor;

    impl Executor for DummyExecutor {
        fn execute(&self, _request: &ExecutionRequest) -> Result<ExecutionResult, ExecutionError> {
            todo!("dummy executor")
        }
    }

    #[test]
    fn registry_starts_empty() {
        let registry = ExecutorRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn register_and_lookup() {
        let mut registry = ExecutorRegistry::new();
        registry.register(
            ExecutionActionDiscriminant::CommandRun,
            Box::new(DummyExecutor),
        );

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());

        let action = ExecutionAction::CommandRun {
            command: "ls".into(),
            working_directory: PathBuf::from("."),
        };
        assert!(registry.get(&action).is_some());
    }

    #[test]
    fn lookup_missing_returns_none() {
        let registry = ExecutorRegistry::new();
        let action = ExecutionAction::FileDelete {
            path: PathBuf::from("/tmp/file"),
            working_directory: PathBuf::from("."),
        };
        assert!(registry.get(&action).is_none());
    }

    #[test]
    fn discriminant_from_action() {
        let cmd = ExecutionAction::CommandRun {
            command: "ls".into(),
            working_directory: PathBuf::from("."),
        };
        let write = ExecutionAction::FileWrite {
            path: PathBuf::from("/tmp/file"),
            content: vec![],
            working_directory: PathBuf::from("."),
        };
        let delete = ExecutionAction::FileDelete {
            path: PathBuf::from("/tmp/file"),
            working_directory: PathBuf::from("."),
        };

        assert_eq!(
            ExecutionActionDiscriminant::from(&cmd),
            ExecutionActionDiscriminant::CommandRun
        );
        assert_eq!(
            ExecutionActionDiscriminant::from(&write),
            ExecutionActionDiscriminant::FileWrite
        );
        assert_eq!(
            ExecutionActionDiscriminant::from(&delete),
            ExecutionActionDiscriminant::FileDelete
        );
    }
}
