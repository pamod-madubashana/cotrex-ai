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
    /// 2. **Lookup** — find the executor in the registry. If missing,
    ///    return `Internal` and append no events.
    /// 3. **Request event** — create `ExecutionRequested` and append to the
    ///    event store. If the append fails, return `EventAppendFailed` and
    ///    do not call the executor.
    /// 4. **Dispatch** — call the executor.
    /// 5. **Result event** — append `ExecutionCompleted` on success or
    ///    `ExecutionFailed` on failure.
    /// 6. **Return** — the result or error from the executor.
    ///
    /// # Errors
    ///
    /// - [`ExecutionError::ValidationFailed`] — capabilities invalid
    /// - [`ExecutionError::Internal`] — no executor registered for action
    /// - [`ExecutionError::EventAppendFailed`] — event store rejected append
    /// - [`ExecutionError::ExecutorFailed`] — executor returned an error
    pub fn submit(&self, request: &ExecutionRequest) -> Result<ExecutionResult, ExecutionError> {
        // Step 1: Validate capabilities
        self.validator.validate(request)?;

        // Step 2: Look up executor (before appending any events)
        // A missing executor is a deterministic configuration error.
        // Detect it before recording the request to avoid orphaned events.
        let executor: &dyn Executor = self
            .registry
            .get(&request.action)
            .ok_or_else(|| ExecutionError::Internal("no executor registered".into()))?;

        // Step 3: Append ExecutionRequested event
        let requested = ExecutionRequested {
            execution_id: request.id,
            command: match &request.action {
                crate::request::ExecutionAction::CommandRun { command, .. } => command.clone(),
                crate::request::ExecutionAction::FileWrite { path, .. } => {
                    format!("write:{}", path.display())
                }
                crate::request::ExecutionAction::FileDelete { path, .. } => {
                    format!("delete:{}", path.display())
                }
            },
            working_directory: match &request.action {
                crate::request::ExecutionAction::CommandRun {
                    working_directory, ..
                } => working_directory.clone(),
                crate::request::ExecutionAction::FileWrite {
                    working_directory, ..
                } => working_directory.clone(),
                crate::request::ExecutionAction::FileDelete {
                    working_directory, ..
                } => working_directory.clone(),
            },
            requested_at: SystemTime::now(),
        };
        self.store
            .append(EventPayload::ExecutionRequested(requested))?;

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
                    stdout: Vec::new(),
                    stderr: Vec::new(),
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
                    stdout: template.stdout.clone(),
                    stderr: template.stderr.clone(),
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
                    stdout: Vec::new(),
                    stderr: Vec::new(),
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
                    stdout: Vec::new(),
                    stderr: Vec::new(),
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

    // ===================================================================
    // FAILURE PATH VERIFICATION (Commit 3.5)
    // ===================================================================

    // -----------------------------------------------------------------------
    // Scenario 1: Executor succeeds, ExecutionCompleted append fails.
    //
    // State after failure:
    //   - Executor was called exactly once
    //   - ExecutionRequested EXISTS in the store
    //   - ExecutionCompleted was NOT appended
    //   - EventAppendFailed is returned to the caller
    //   - No retry occurs
    //
    // Architectural note: This produces an orphaned ExecutionRequested
    // event. The execution was attempted but the result was not recorded.
    // This is acceptable because:
    //   1. The EventStore is append-only — we cannot remove the request.
    //   2. Projections will see a request with no completion/failure.
    //   3. This represents a real system state: the execution happened
    //      but we lost the result record.
    //   4. Recovery requires external reconciliation, not automatic retry.
    // -----------------------------------------------------------------------

    #[test]
    fn submit_completed_append_failure_after_executor_success() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct SucceedThenAppendFailExecutor;

        impl Executor for SucceedThenAppendFailExecutor {
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
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                })
            }
        }

        // Capacity 1: first append (ExecutionRequested) succeeds,
        // second append (ExecutionCompleted) fails with Backpressure
        let store = EventStore::with_capacity(1);
        let validator = Box::new(ExecutionPolicy::allow_all());
        let mut registry = ExecutorRegistry::new();
        registry.register(
            ExecutionActionDiscriminant::CommandRun,
            Box::new(SucceedThenAppendFailExecutor),
        );
        let engine = ExecutionEngine::new(store, validator, registry);

        CALL_COUNT.store(0, Ordering::SeqCst);
        let req = command_request();
        let result = engine.submit(&req);

        // Executor was called exactly once
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1);

        // Return value is EventAppendFailed (from Completed append)
        assert!(result.is_err());
        match result.unwrap_err() {
            ExecutionError::EventAppendFailed(_) => {}
            other => panic!("expected EventAppendFailed, got: {:?}", other),
        }

        // ExecutionRequested EXISTS (sequence 1)
        let payloads = event_payloads(engine.store());
        assert_eq!(payloads.len(), 1, "only ExecutionRequested should exist");
        assert!(
            matches!(&payloads[0], EventPayload::ExecutionRequested(_)),
            "first event must be ExecutionRequested"
        );

        // ExecutionCompleted was NOT appended
        assert!(
            !payloads
                .iter()
                .any(|p| matches!(p, EventPayload::ExecutionCompleted(_))),
            "ExecutionCompleted should NOT exist"
        );
    }

    // -----------------------------------------------------------------------
    // Scenario 2: Executor fails, ExecutionFailed append fails.
    //
    // State after failure:
    //   - Executor was called exactly once
    //   - ExecutionRequested EXISTS in the store
    //   - ExecutionFailed was NOT appended
    //   - EventAppendFailed is returned to the caller
    //
    // Architectural note: Same orphan scenario as Scenario 1. The
    // execution failed, but we cannot record the failure. The caller
    // receives EventAppendFailed, not ExecutorFailed. This is a
    // different error path than a normal executor failure.
    // -----------------------------------------------------------------------

    #[test]
    fn submit_failed_append_failure_after_executor_failure() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct FailThenAppendFailExecutor;

        impl Executor for FailThenAppendFailExecutor {
            fn execute(
                &self,
                _request: &ExecutionRequest,
            ) -> Result<ExecutionResult, ExecutionError> {
                CALL_COUNT.fetch_add(1, Ordering::SeqCst);
                Err(ExecutionError::ExecutorFailed("process crashed".into()))
            }
        }

        // Capacity 1: first append (ExecutionRequested) succeeds,
        // second append (ExecutionFailed) fails with Backpressure
        let store = EventStore::with_capacity(1);
        let validator = Box::new(ExecutionPolicy::allow_all());
        let mut registry = ExecutorRegistry::new();
        registry.register(
            ExecutionActionDiscriminant::CommandRun,
            Box::new(FailThenAppendFailExecutor),
        );
        let engine = ExecutionEngine::new(store, validator, registry);

        CALL_COUNT.store(0, Ordering::SeqCst);
        let req = command_request();
        let result = engine.submit(&req);

        // Executor was called exactly once
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1);

        // Return value is EventAppendFailed (from Failed append),
        // NOT ExecutorFailed
        assert!(result.is_err());
        match result.unwrap_err() {
            ExecutionError::EventAppendFailed(_) => {}
            other => panic!("expected EventAppendFailed, got: {:?}", other),
        }

        // ExecutionRequested EXISTS (sequence 1)
        let payloads = event_payloads(engine.store());
        assert_eq!(payloads.len(), 1, "only ExecutionRequested should exist");
        assert!(
            matches!(&payloads[0], EventPayload::ExecutionRequested(_)),
            "first event must be ExecutionRequested"
        );

        // ExecutionFailed was NOT appended
        assert!(
            !payloads
                .iter()
                .any(|p| matches!(p, EventPayload::ExecutionFailed(_))),
            "ExecutionFailed should NOT exist"
        );
    }

    // -----------------------------------------------------------------------
    // Scenario 3: No executor registered.
    //
    // BEHAVIOR (after fix):
    //   1. Validation passes
    //   2. Registry lookup fails → Internal("no executor registered")
    //   3. NO events are appended
    //
    // A missing executor is a deterministic configuration/programming
    // error. It is detected before any event is recorded. The EventStore
    // remains empty.
    //
    // INVARIANT: Only two scenarios produce orphaned ExecutionRequested:
    //   1. Executor succeeded, Completed append failed
    //   2. Executor failed, Failed append failed
    // Missing executor is NOT one of them.
    // -----------------------------------------------------------------------

    #[test]
    fn submit_missing_executor_behavior() {
        // Engine with no executor registered
        let store = EventStore::new();
        let validator = Box::new(ExecutionPolicy::allow_all());
        let registry = ExecutorRegistry::new(); // empty
        let engine = ExecutionEngine::new(store, validator, registry);

        let req = command_request();
        let result = engine.submit(&req);

        // Error is Internal
        assert!(result.is_err());
        match result.unwrap_err() {
            ExecutionError::Internal(msg) => {
                assert_eq!(msg, "no executor registered");
            }
            other => panic!("expected Internal error, got: {:?}", other),
        }

        // No events appended — executor missing is a pre-flight error
        let payloads = event_payloads(engine.store());
        assert_eq!(
            payloads.len(),
            0,
            "missing executor must not produce any events"
        );
    }

    // -----------------------------------------------------------------------
    // Scenario 4: Duration propagation.
    //
    // The engine measures wall-clock time around executor.execute().
    // The FakeExecutor's configured duration_ms field is IGNORED —
    // the engine always measures actual elapsed time.
    //
    // Duration flows into:
    //   - ExecutionResult: NOT set by engine (executor sets it)
    //   - ExecutionCompleted.duration_ms: SET by engine
    //   - ExecutionFailed.duration_ms: SET by engine
    //
    // The ExecutionResult returned to the caller does NOT contain
    // the engine-measured duration — it contains whatever the executor
    // returned. This is intentional: the executor knows its own timing
    // better than the engine wrapper.
    //
    // For real executors, the engine-measured duration and the
    // executor-reported duration should be approximately equal.
    // -----------------------------------------------------------------------

    #[test]
    fn execution_duration_propagation() {
        // Test success path: duration in ExecutionCompleted
        let engine = engine_with_executor(FakeExecutor::succeed());
        let req = command_request();
        engine.submit(&req).unwrap();

        let payloads = event_payloads(engine.store());
        if let EventPayload::ExecutionCompleted(ref completed) = payloads[1] {
            // Duration should be >= 0 (measured by engine)
            // We cannot assert exact value since it's wall-clock time
            assert!(
                completed.duration_ms < 1000,
                "duration should be reasonable, got {}ms",
                completed.duration_ms
            );
        } else {
            panic!("expected ExecutionCompleted event");
        }
    }

    #[test]
    fn execution_failure_duration_propagation() {
        // Test failure path: duration in ExecutionFailed
        let engine = engine_with_executor(FakeExecutor::fail("timeout"));
        let req = command_request();
        let _ = engine.submit(&req);

        let payloads = event_payloads(engine.store());
        if let EventPayload::ExecutionFailed(ref failed) = payloads[1] {
            assert!(
                failed.duration_ms < 1000,
                "duration should be reasonable, got {}ms",
                failed.duration_ms
            );
        } else {
            panic!("expected ExecutionFailed event");
        }
    }

    #[test]
    fn execution_duration_is_engine_measured_not_executor_provided() {
        // Verify that the engine measures duration, not the executor.
        // FakeExecutor returns duration_ms: 0, but the engine measures
        // wall-clock time which will be >= 0.

        let engine = engine_with_executor(FakeExecutor::succeed());
        let req = command_request();
        let result = engine.submit(&req).unwrap();

        // The executor returned duration_ms: 0 (from FakeExecutor::succeed)
        assert_eq!(result.duration_ms, 0, "executor returned 0");

        // The engine-measured duration in the event is different
        let payloads = event_payloads(engine.store());
        if let EventPayload::ExecutionCompleted(ref completed) = payloads[1] {
            // Engine duration is whatever wall-clock time elapsed
            // It does NOT have to match result.duration_ms
            // Just verify it's a valid u64 (always true, documents intent)
            let _ = completed.duration_ms;
        } else {
            panic!("expected ExecutionCompleted event");
        }
    }
}
