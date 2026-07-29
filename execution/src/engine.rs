use crate::error::ExecutionError;
use crate::executor::Executor;
use crate::policy::CapabilityValidator;
use crate::registry::ExecutorRegistry;
use crate::request::ExecutionRequest;
use crate::result::ExecutionResult;
use kernel::EventStore;

// ---------------------------------------------------------------------------
// Execution Engine (RFC-0004)
// ---------------------------------------------------------------------------

/// Orchestrates execution of actions in the external world.
///
/// The engine owns the submit flow:
///
/// 1. Validate capabilities via [`CapabilityValidator`]
/// 2. Append `ExecutionRequested` event to [`EventStore`]
/// 3. Dispatch to the appropriate [`Executor`] via [`ExecutorRegistry`]
/// 4. Append `ExecutionCompleted` or `ExecutionFailed` event
/// 5. Return [`ExecutionResult`]
///
/// # Responsibility
///
/// - Orchestrate the execution lifecycle
/// - Coordinate collaborators (validator, registry, event store)
///
/// # Non-responsibility
///
/// - Spawning commands directly
/// - Parsing shell output
/// - Updating projections
/// - Interpreting file contents
/// - Assigning event IDs or sequence numbers
pub struct ExecutionEngine {
    store: EventStore,
    validator: Box<dyn CapabilityValidator>,
    registry: ExecutorRegistry,
}

impl ExecutionEngine {
    /// Create a new execution engine.
    ///
    /// Takes ownership of the event store, a capability validator,
    /// and an executor registry.
    pub fn new(
        store: EventStore,
        validator: Box<dyn CapabilityValidator>,
        registry: ExecutorRegistry,
    ) -> Self {
        Self {
            store,
            validator,
            registry,
        }
    }

    /// Submit an execution request.
    ///
    /// Orchestrates the full execution flow: validate → request event →
    /// dispatch → result event → return result.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError`] if validation fails, event append fails,
    /// or no executor is registered for the action.
    pub fn submit(&self, request: &ExecutionRequest) -> Result<ExecutionResult, ExecutionError> {
        // Step 1: Validate capabilities
        self.validator.validate(request)?;

        // Step 2: Append ExecutionRequested event
        // TODO: Create ExecutionRequested from request and append to self.store
        let _ = &self.store;

        // Step 3: Look up executor
        let _executor: &dyn Executor = self
            .registry
            .get(&request.action)
            .ok_or_else(|| ExecutionError::Internal("no executor registered".into()))?;

        // Step 4: Dispatch execution
        // TODO: Call executor.execute(request) and record timing
        todo!("dispatch executor and record result events")
    }

    /// Return a reference to the underlying event store.
    pub fn store(&self) -> &EventStore {
        &self.store
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::ExecutionPolicy;
    use crate::registry::ExecutorRegistry;
    use crate::request::{Capability, ExecutionAction};
    use std::path::PathBuf;

    fn test_engine() -> ExecutionEngine {
        let store = EventStore::new();
        let validator = Box::new(ExecutionPolicy::allow_all());
        let registry = ExecutorRegistry::new();
        ExecutionEngine::new(store, validator, registry)
    }

    #[test]
    fn engine_holds_store_reference() {
        let engine = test_engine();
        assert_eq!(engine.store().len(), 0);
    }

    #[test]
    fn submit_rejects_invalid_capabilities() {
        let store = EventStore::new();
        let validator = Box::new(ExecutionPolicy::deny_all());
        let registry = ExecutorRegistry::new();
        let engine = ExecutionEngine::new(store, validator, registry);

        let req = ExecutionRequest::new(
            ExecutionAction::CommandRun {
                command: "ls".into(),
                working_directory: PathBuf::from("."),
            },
            vec![Capability::CommandRun],
        );

        let result = engine.submit(&req);
        assert!(result.is_err());
    }

    #[test]
    fn submit_rejects_missing_executor() {
        let store = EventStore::new();
        let validator = Box::new(ExecutionPolicy::allow_all());
        let registry = ExecutorRegistry::new();
        let engine = ExecutionEngine::new(store, validator, registry);

        let req = ExecutionRequest::new(
            ExecutionAction::CommandRun {
                command: "ls".into(),
                working_directory: PathBuf::from("."),
            },
            vec![Capability::CommandRun],
        );

        // Validation passes, but no executor is registered
        // Registry lookup returns None → ExecutionError::Internal
        let result = engine.submit(&req);
        assert!(result.is_err());
    }
}
