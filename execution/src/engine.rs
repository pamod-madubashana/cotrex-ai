use crate::error::ExecutionError;
use crate::executor::Executor;
use crate::policy::CapabilityValidator;
use crate::registry::ExecutorRegistry;
use crate::request::ExecutionRequest;
use crate::result::ExecutionResult;
use kernel::event::{ExecutionCompleted, ExecutionFailed, ExecutionRequested};
use kernel::{EventPayload, EventStore};
use std::time::SystemTime;

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
    /// Orchestrates the full execution flow:
    ///
    /// 1. **Validate** — check capabilities via the validator. If invalid,
    ///    return `ValidationFailed` and append no events.
    /// 2. **Request event** — create `ExecutionRequested` and append to the
    ///    event store. If the append fails, return `EventAppendFailed` and
    ///    do not call the executor.
    /// 3. **Dispatch** — look up the executor in the registry and call it.
    /// 4. **Result event** — append `ExecutionCompleted` on success or
    ///    `ExecutionFailed` on failure.
    /// 5. **Return** — the result or error from the executor.
    ///
    /// # Errors
    ///
    /// - [`ExecutionError::ValidationFailed`] — capabilities invalid
    /// - [`ExecutionError::EventAppendFailed`] — event store rejected append
    /// - [`ExecutionError::ExecutorFailed`] — executor returned an error
    /// - [`ExecutionError::Internal`] — no executor registered for action
    pub fn submit(&self, request: &ExecutionRequest) -> Result<ExecutionResult, ExecutionError> {
        // Step 1: Validate capabilities
        self.validator.validate(request)?;

        // Step 2: Append ExecutionRequested event
        let requested = ExecutionRequested {
            execution_id: request.id,
            command: match &request.action {
                crate::request::ExecutionAction::CommandRun { command, .. } => command.clone(),
                crate::request::ExecutionAction::FileWrite { path, .. } => {
                    format!("write:{}", path.display())
                }
                crate::request::ExecutionAction::FileDelete { path } => {
                    format!("delete:{}", path.display())
                }
            },
            working_directory: match &request.action {
                crate::request::ExecutionAction::CommandRun {
                    working_directory, ..
                } => working_directory.clone(),
                crate::request::ExecutionAction::FileWrite { path, .. } => path.clone(),
                crate::request::ExecutionAction::FileDelete { path } => path.clone(),
            },
            requested_at: SystemTime::now(),
        };
        self.store
            .append(EventPayload::ExecutionRequested(requested))?;

        // Step 3: Look up executor
        let executor: &dyn Executor = self
            .registry
            .get(&request.action)
            .ok_or_else(|| ExecutionError::Internal("no executor registered".into()))?;

        // Step 4: Dispatch execution
        let start = SystemTime::now();
        let exec_result = executor.execute(request);
        let duration_ms = start.elapsed().map(|d| d.as_millis() as u64).unwrap_or(0);

        // Step 5: Append result event and return
        match exec_result {
            Ok(result) => {
                let completed = ExecutionCompleted {
                    execution_id: request.id,
                    exit_code: result.exit_code.unwrap_or(0),
                    duration_ms,
                    completed_at: SystemTime::now(),
                };
                self.store
                    .append(EventPayload::ExecutionCompleted(completed))?;
                Ok(result)
            }
            Err(ExecutionError::ExecutorFailed(reason)) => {
                let failed = ExecutionFailed {
                    execution_id: request.id,
                    error: reason.clone(),
                    duration_ms,
                    failed_at: SystemTime::now(),
                };
                self.store.append(EventPayload::ExecutionFailed(failed))?;
                Err(ExecutionError::ExecutorFailed(reason))
            }
            Err(other) => Err(other),
        }
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
    use crate::registry::{ExecutionActionDiscriminant, ExecutorRegistry};
    use crate::request::{Capability, ExecutionAction};
    use kernel::EventPayload;
    use std::path::PathBuf;

    // -----------------------------------------------------------------------
    // FakeExecutor — deterministic test double
    // -----------------------------------------------------------------------

    /// A configurable fake executor for testing orchestration.
    ///
    /// Returns a preconfigured result without touching the filesystem
    /// or spawning any processes.
    struct FakeExecutor {
        result: Result<ExecutionResult, ExecutionError>,
    }

    impl FakeExecutor {
        fn succeed() -> Self {
            Self {
                result: Ok(ExecutionResult {
                    execution_id: uuid::Uuid::new_v4(),
                    success: true,
                    exit_code: Some(0),
                    duration_ms: 0,
                    error: None,
                }),
            }
        }

        fn fail(reason: &str) -> Self {
            Self {
                result: Err(ExecutionError::ExecutorFailed(reason.into())),
            }
        }
    }

    impl Executor for FakeExecutor {
        fn execute(&self, request: &ExecutionRequest) -> Result<ExecutionResult, ExecutionError> {
            match &self.result {
                Ok(template) => Ok(ExecutionResult {
                    execution_id: request.id,
                    success: template.success,
                    exit_code: template.exit_code,
                    duration_ms: template.duration_ms,
                    error: template.error.clone(),
                }),
                Err(ExecutionError::ExecutorFailed(reason)) => {
                    Err(ExecutionError::ExecutorFailed(reason.clone()))
                }
                Err(other) => Err(ExecutionError::Internal(format!("{:?}", other))),
            }
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn command_request() -> ExecutionRequest {
        ExecutionRequest::new(
            ExecutionAction::CommandRun {
                command: "echo hello".into(),
                working_directory: PathBuf::from("/project"),
            },
            vec![Capability::CommandRun],
        )
    }

    fn engine_with_executor(executor: FakeExecutor) -> ExecutionEngine {
        let store = EventStore::new();
        let validator = Box::new(ExecutionPolicy::allow_all());
        let mut registry = ExecutorRegistry::new();
        registry.register(ExecutionActionDiscriminant::CommandRun, Box::new(executor));
        ExecutionEngine::new(store, validator, registry)
    }

    fn engine_denied() -> ExecutionEngine {
        let store = EventStore::new();
        let validator = Box::new(ExecutionPolicy::deny_all());
        let registry = ExecutorRegistry::new();
        ExecutionEngine::new(store, validator, registry)
    }

    fn event_payloads(store: &EventStore) -> Vec<EventPayload> {
        store
            .replay(1)
            .unwrap()
            .events
            .into_iter()
            .map(|e| e.payload)
            .collect()
    }

    // -----------------------------------------------------------------------
    // Test 1: submit_valid_request_appends_requested_event
    // -----------------------------------------------------------------------

    #[test]
    fn submit_valid_request_appends_requested_event() {
        let engine = engine_with_executor(FakeExecutor::succeed());
        let req = command_request();
        engine.submit(&req).unwrap();

        let payloads = event_payloads(engine.store());
        assert!(!payloads.is_empty());
        assert!(matches!(&payloads[0], EventPayload::ExecutionRequested(_)));
    }

    // -----------------------------------------------------------------------
    // Test 2: submit_success_appends_completed_event
    // -----------------------------------------------------------------------

    #[test]
    fn submit_success_appends_completed_event() {
        let engine = engine_with_executor(FakeExecutor::succeed());
        let req = command_request();
        engine.submit(&req).unwrap();

        let payloads = event_payloads(engine.store());
        assert_eq!(payloads.len(), 2);
        assert!(matches!(&payloads[1], EventPayload::ExecutionCompleted(_)));
    }

    // -----------------------------------------------------------------------
    // Test 3: submit_executor_failure_appends_failed_event
    // -----------------------------------------------------------------------

    #[test]
    fn submit_executor_failure_appends_failed_event() {
        let engine = engine_with_executor(FakeExecutor::fail("command crashed"));
        let req = command_request();
        let result = engine.submit(&req);
        assert!(result.is_err());

        let payloads = event_payloads(engine.store());
        assert_eq!(payloads.len(), 2);
        assert!(matches!(&payloads[1], EventPayload::ExecutionFailed(_)));
    }

    // -----------------------------------------------------------------------
    // Test 4: submit_validation_failure_appends_no_events
    // -----------------------------------------------------------------------

    #[test]
    fn submit_validation_failure_appends_no_events() {
        let engine = engine_denied();
        let req = command_request();
        let result = engine.submit(&req);
        assert!(result.is_err());

        assert_eq!(engine.store().len(), 0);
    }

    // -----------------------------------------------------------------------
    // Test 5: submit_eventstore_failure_does_not_execute
    // -----------------------------------------------------------------------

    #[test]
    fn submit_eventstore_failure_does_not_execute() {
        use crate::executor::Executor;
        use std::sync::atomic::{AtomicBool, Ordering};

        static EXECUTOR_CALLED: AtomicBool = AtomicBool::new(false);

        struct TrackExecutor;

        impl Executor for TrackExecutor {
            fn execute(
                &self,
                _request: &ExecutionRequest,
            ) -> Result<ExecutionResult, ExecutionError> {
                EXECUTOR_CALLED.store(true, Ordering::SeqCst);
                Ok(ExecutionResult {
                    execution_id: uuid::Uuid::new_v4(),
                    success: true,
                    exit_code: Some(0),
                    duration_ms: 0,
                    error: None,
                })
            }
        }

        // Create a store at capacity 1, append one event to fill it
        let store = EventStore::with_capacity(1);
        store
            .append(EventPayload::ExecutionRequested(ExecutionRequested {
                execution_id: uuid::Uuid::new_v4(),
                command: "fill".into(),
                working_directory: PathBuf::from("."),
                requested_at: SystemTime::now(),
            }))
            .unwrap();

        let validator = Box::new(ExecutionPolicy::allow_all());
        let mut registry = ExecutorRegistry::new();
        registry.register(
            ExecutionActionDiscriminant::CommandRun,
            Box::new(TrackExecutor),
        );
        let engine = ExecutionEngine::new(store, validator, registry);

        EXECUTOR_CALLED.store(false, Ordering::SeqCst);
        let req = command_request();
        let result = engine.submit(&req);

        // Append should fail (backpressure), executor should NOT be called
        assert!(result.is_err());
        assert!(!EXECUTOR_CALLED.load(Ordering::SeqCst));
    }

    // -----------------------------------------------------------------------
    // Test 6: executor_called_exactly_once
    // -----------------------------------------------------------------------

    #[test]
    fn executor_called_exactly_once() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct CountExecutor;

        impl Executor for CountExecutor {
            fn execute(
                &self,
                request: &ExecutionRequest,
            ) -> Result<ExecutionResult, ExecutionError> {
                CALL_COUNT.fetch_add(1, Ordering::SeqCst);
                Ok(ExecutionResult {
                    execution_id: request.id,
                    success: true,
                    exit_code: Some(0),
                    duration_ms: 0,
                    error: None,
                })
            }
        }

        let store = EventStore::new();
        let validator = Box::new(ExecutionPolicy::allow_all());
        let mut registry = ExecutorRegistry::new();
        registry.register(
            ExecutionActionDiscriminant::CommandRun,
            Box::new(CountExecutor),
        );
        let engine = ExecutionEngine::new(store, validator, registry);

        CALL_COUNT.store(0, Ordering::SeqCst);
        let req = command_request();
        engine.submit(&req).unwrap();

        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1);
    }

    // -----------------------------------------------------------------------
    // Test 7: execution_requested_before_completed
    // -----------------------------------------------------------------------

    #[test]
    fn execution_requested_before_completed() {
        let engine = engine_with_executor(FakeExecutor::succeed());
        let req = command_request();
        engine.submit(&req).unwrap();

        let payloads = event_payloads(engine.store());
        assert_eq!(payloads.len(), 2);

        // First event must be ExecutionRequested
        let is_requested = matches!(&payloads[0], EventPayload::ExecutionRequested(_));
        assert!(is_requested, "first event must be ExecutionRequested");

        // Second event must be ExecutionCompleted
        let is_completed = matches!(&payloads[1], EventPayload::ExecutionCompleted(_));
        assert!(is_completed, "second event must be ExecutionCompleted");
    }

    // -----------------------------------------------------------------------
    // Test 8: execution_requested_before_failed
    // -----------------------------------------------------------------------

    #[test]
    fn execution_requested_before_failed() {
        let engine = engine_with_executor(FakeExecutor::fail("timeout"));
        let req = command_request();
        let _ = engine.submit(&req);

        let payloads = event_payloads(engine.store());
        assert_eq!(payloads.len(), 2);

        // First event must be ExecutionRequested
        let is_requested = matches!(&payloads[0], EventPayload::ExecutionRequested(_));
        assert!(is_requested, "first event must be ExecutionRequested");

        // Second event must be ExecutionFailed
        let is_failed = matches!(&payloads[1], EventPayload::ExecutionFailed(_));
        assert!(is_failed, "second event must be ExecutionFailed");
    }

    // -----------------------------------------------------------------------
    // Test 9: returned_result_matches_completed_event
    // -----------------------------------------------------------------------

    #[test]
    fn returned_result_matches_completed_event() {
        let engine = engine_with_executor(FakeExecutor::succeed());
        let req = command_request();
        let result = engine.submit(&req).unwrap();

        let payloads = event_payloads(engine.store());
        if let EventPayload::ExecutionCompleted(ref completed) = payloads[1] {
            assert_eq!(completed.execution_id, result.execution_id);
            assert_eq!(completed.exit_code, result.exit_code.unwrap_or(0));
        } else {
            panic!("expected ExecutionCompleted event");
        }
    }

    // -----------------------------------------------------------------------
    // Test 10: returned_error_matches_failed_event
    // -----------------------------------------------------------------------

    #[test]
    fn returned_error_matches_failed_event() {
        let engine = engine_with_executor(FakeExecutor::fail("segfault"));
        let req = command_request();
        let err = engine.submit(&req).unwrap_err();

        let payloads = event_payloads(engine.store());
        if let EventPayload::ExecutionFailed(ref failed) = payloads[1] {
            assert_eq!(failed.execution_id, req.id);
            match &err {
                ExecutionError::ExecutorFailed(reason) => assert_eq!(&failed.error, reason),
                _ => panic!("expected ExecutorFailed error"),
            }
        } else {
            panic!("expected ExecutionFailed event");
        }
    }
}
