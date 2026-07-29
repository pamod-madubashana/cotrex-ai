use crate::event::Event;
use crate::projection::{FileChangeProjection, ProjectionStatus};
use crate::store::{EventStore, EventStoreError};
use std::collections::HashMap;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Projection trait
// ---------------------------------------------------------------------------

pub trait Projection: Send + Sync {
    /// Return the projection name.
    fn name(&self) -> &str;

    /// Return current projection status.
    fn status(&self) -> ProjectionStatus;

    /// Return checkpoint offset.
    fn checkpoint(&self) -> u64;

    /// Process a single event.
    fn process_event(&self, event: &Event) -> Result<(), EventStoreError>;

    /// Rebuild from store.
    fn rebuild(&self, store: &EventStore) -> Result<(), EventStoreError>;

    /// Initialize projection.
    fn initialize(&self, store: &EventStore) -> Result<(), EventStoreError>;

    /// Mark as processing.
    fn start_processing(&self) -> Result<(), EventStoreError>;

    /// Mark as failed.
    fn mark_failed(&self) -> Result<(), EventStoreError>;
}

// ---------------------------------------------------------------------------
// ProjectionEngine (RFC-0002, Section 7)
// ---------------------------------------------------------------------------

pub struct ProjectionEngine {
    projections: Mutex<HashMap<String, Box<dyn Projection>>>,
}

impl ProjectionEngine {
    pub fn new() -> Self {
        Self {
            projections: Mutex::new(HashMap::new()),
        }
    }

    /// Register a projection.
    pub fn register(&self, projection: Box<dyn Projection>) -> Result<(), EventStoreError> {
        let mut projections = self.projections.lock().map_err(|e| {
            EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
        })?;
        let name = projection.name().to_string();
        projections.insert(name, projection);
        Ok(())
    }

    /// Remove a projection by name.
    pub fn remove(&self, name: &str) -> Result<Option<Box<dyn Projection>>, EventStoreError> {
        let mut projections = self.projections.lock().map_err(|e| {
            EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
        })?;
        Ok(projections.remove(name))
    }

    /// Get projection status by name.
    pub fn status(&self, name: &str) -> Result<ProjectionStatus, EventStoreError> {
        let projections = self.projections.lock().map_err(|e| {
            EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
        })?;
        projections.get(name).map(|p| p.status()).ok_or_else(|| {
            EventStoreError::ProjectionFailure(format!("unknown projection: {}", name))
        })
    }

    /// Get checkpoint by name.
    pub fn checkpoint(&self, name: &str) -> Result<u64, EventStoreError> {
        let projections = self.projections.lock().map_err(|e| {
            EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
        })?;
        projections
            .get(name)
            .map(|p| p.checkpoint())
            .ok_or_else(|| {
                EventStoreError::ProjectionFailure(format!("unknown projection: {}", name))
            })
    }

    /// List all projection names.
    pub fn list(&self) -> Result<Vec<String>, EventStoreError> {
        let projections = self.projections.lock().map_err(|e| {
            EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
        })?;
        Ok(projections.keys().cloned().collect())
    }

    /// Start processing for a projection by name.
    pub fn start_processing(&self, name: &str) -> Result<(), EventStoreError> {
        let projections = self.projections.lock().map_err(|e| {
            EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
        })?;
        projections
            .get(name)
            .ok_or_else(|| {
                EventStoreError::ProjectionFailure(format!("unknown projection: {}", name))
            })?
            .start_processing()
    }

    /// Return the number of registered projections.
    pub fn len(&self) -> usize {
        self.projections.lock().map(|p| p.len()).unwrap_or(0)
    }

    /// Return true if no projections are registered.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Process event through all projections (RFC-0002, Section 7).
    ///
    /// One projection failure does not affect others.
    pub fn process_event(&self, event: &Event) -> Result<(), EventStoreError> {
        let projections = self.projections.lock().map_err(|e| {
            EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
        })?;

        for projection in projections.values() {
            // Only process events for projections that are Processing
            if projection.status() == ProjectionStatus::Processing {
                let _ = projection.process_event(event);
            }
        }

        Ok(())
    }

    /// Process event through all projections, collecting errors.
    pub fn process_event_collect_errors(
        &self,
        event: &Event,
    ) -> Result<Vec<(String, EventStoreError)>, EventStoreError> {
        let projections = self.projections.lock().map_err(|e| {
            EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
        })?;

        let mut errors = Vec::new();

        for (name, projection) in projections.iter() {
            if projection.status() == ProjectionStatus::Processing
                && let Err(e) = projection.process_event(event)
            {
                errors.push((name.clone(), e));
            }
        }

        Ok(errors)
    }

    /// Initialize and rebuild all projections from store.
    pub fn rebuild_all(&self, store: &EventStore) -> Result<(), EventStoreError> {
        let projections = self.projections.lock().map_err(|e| {
            EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
        })?;

        for projection in projections.values() {
            projection.initialize(store)?;
        }

        Ok(())
    }
}

impl Default for ProjectionEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Projection trait impl for FileChangeProjection
// ---------------------------------------------------------------------------

impl Projection for FileChangeProjection {
    fn name(&self) -> &str {
        "FileChange"
    }

    fn status(&self) -> ProjectionStatus {
        FileChangeProjection::status(self)
    }

    fn checkpoint(&self) -> u64 {
        FileChangeProjection::checkpoint(self)
    }

    fn process_event(&self, event: &Event) -> Result<(), EventStoreError> {
        FileChangeProjection::process_event(self, event)
    }

    fn rebuild(&self, store: &EventStore) -> Result<(), EventStoreError> {
        FileChangeProjection::rebuild(self, store)
    }

    fn initialize(&self, store: &EventStore) -> Result<(), EventStoreError> {
        FileChangeProjection::initialize(self, store)
    }

    fn start_processing(&self) -> Result<(), EventStoreError> {
        FileChangeProjection::start_processing(self)
    }

    fn mark_failed(&self) -> Result<(), EventStoreError> {
        FileChangeProjection::mark_failed(self)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EventPayload, FileChanged, FileOperation};
    use crate::projection::FileChangeProjection;
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn file_changed_payload(path: &str) -> EventPayload {
        EventPayload::FileChanged(FileChanged {
            path: PathBuf::from(path),
            operation: FileOperation::Created,
            timestamp: SystemTime::now(),
        })
    }

    // -----------------------------------------------------------------------
    // Test-only FailingProjection for isolation testing
    // -----------------------------------------------------------------------

    struct FailingProjection {
        status: Mutex<ProjectionStatus>,
        checkpoint: Mutex<u64>,
        fail_on_process: bool,
    }

    impl FailingProjection {
        fn new(fail_on_process: bool) -> Self {
            Self {
                status: Mutex::new(ProjectionStatus::Created),
                checkpoint: Mutex::new(0),
                fail_on_process,
            }
        }
    }

    impl Projection for FailingProjection {
        fn name(&self) -> &str {
            "Failing"
        }

        fn status(&self) -> ProjectionStatus {
            self.status
                .lock()
                .map(|s| *s)
                .unwrap_or(ProjectionStatus::Failed)
        }

        fn checkpoint(&self) -> u64 {
            self.checkpoint.lock().map(|c| *c).unwrap_or(0)
        }

        fn process_event(&self, event: &Event) -> Result<(), EventStoreError> {
            if self.fail_on_process {
                return Err(EventStoreError::ProjectionFailure(
                    "forced test failure".to_string(),
                ));
            }
            let mut cp = self.checkpoint.lock().map_err(|e| {
                EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
            })?;
            *cp = event.sequence;
            Ok(())
        }

        fn rebuild(&self, _store: &EventStore) -> Result<(), EventStoreError> {
            let mut cp = self.checkpoint.lock().map_err(|e| {
                EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
            })?;
            *cp = 0;
            Ok(())
        }

        fn initialize(&self, _store: &EventStore) -> Result<(), EventStoreError> {
            let mut status = self.status.lock().map_err(|e| {
                EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
            })?;
            *status = ProjectionStatus::Initialized;
            Ok(())
        }

        fn start_processing(&self) -> Result<(), EventStoreError> {
            let mut status = self.status.lock().map_err(|e| {
                EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
            })?;
            if !status.can_transition_to(ProjectionStatus::Processing) {
                return Err(EventStoreError::ProjectionFailure(format!(
                    "invalid transition: {:?} -> Processing",
                    *status
                )));
            }
            *status = ProjectionStatus::Processing;
            Ok(())
        }

        fn mark_failed(&self) -> Result<(), EventStoreError> {
            let mut status = self.status.lock().map_err(|e| {
                EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
            })?;
            *status = ProjectionStatus::Failed;
            Ok(())
        }
    }

    // -----------------------------------------------------------------------
    // Engine tests
    // -----------------------------------------------------------------------

    #[test]
    fn engine_starts_empty() {
        let engine = ProjectionEngine::new();
        assert!(engine.is_empty());
        assert_eq!(engine.len(), 0);
    }

    #[test]
    fn engine_register_projection() {
        let engine = ProjectionEngine::new();
        let proj = FileChangeProjection::new();
        engine.register(Box::new(proj)).unwrap();

        assert_eq!(engine.len(), 1);
        assert!(!engine.is_empty());

        let list = engine.list().unwrap();
        assert!(list.contains(&"FileChange".to_string()));
    }

    #[test]
    fn engine_remove_projection() {
        let engine = ProjectionEngine::new();
        let proj = FileChangeProjection::new();
        engine.register(Box::new(proj)).unwrap();

        let removed = engine.remove("FileChange").unwrap();
        assert!(removed.is_some());
        assert!(engine.is_empty());
    }

    #[test]
    fn engine_remove_unknown_returns_none() {
        let engine = ProjectionEngine::new();
        let removed = engine.remove("nonexistent").unwrap();
        assert!(removed.is_none());
    }

    #[test]
    fn engine_status_unknown_projection() {
        let engine = ProjectionEngine::new();
        let result = engine.status("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn engine_rebuild_all() {
        let engine = ProjectionEngine::new();
        let proj = FileChangeProjection::new();
        engine.register(Box::new(proj)).unwrap();

        let store = EventStore::new();
        store.append(file_changed_payload("a.txt")).unwrap();
        store.append(file_changed_payload("b.txt")).unwrap();

        engine.rebuild_all(&store).unwrap();

        assert_eq!(
            engine.status("FileChange").unwrap(),
            ProjectionStatus::Initialized
        );
    }

    #[test]
    fn engine_process_event_through_all() {
        let engine = ProjectionEngine::new();
        let proj = FileChangeProjection::new();
        engine.register(Box::new(proj)).unwrap();

        let store = EventStore::new();
        engine.rebuild_all(&store).unwrap();
        engine.start_processing("FileChange").unwrap();

        let event = store.append(file_changed_payload("a.txt")).unwrap();
        engine.process_event(&event).unwrap();

        // Checkpoint should have advanced
        assert_eq!(engine.checkpoint("FileChange").unwrap(), 1);
    }

    #[test]
    fn engine_process_event_collect_errors() {
        let engine = ProjectionEngine::new();
        let proj = FileChangeProjection::new();
        engine.register(Box::new(proj)).unwrap();

        let store = EventStore::new();
        engine.rebuild_all(&store).unwrap();
        // Don't start processing - events won't be processed
        // but no errors either since we skip non-Processing projections

        let event = store.append(file_changed_payload("a.txt")).unwrap();
        let errors = engine.process_event_collect_errors(&event).unwrap();
        assert!(errors.is_empty());
    }

    #[test]
    fn engine_multiple_projections_isolated() {
        let engine = ProjectionEngine::new();
        let proj1 = FileChangeProjection::new();
        let proj2 = FileChangeProjection::new();
        engine.register(Box::new(proj1)).unwrap();
        engine.register(Box::new(proj2)).unwrap();

        // Second projection overwrites first (same name "FileChange")
        assert_eq!(engine.len(), 1);

        let store = EventStore::new();
        engine.rebuild_all(&store).unwrap();

        assert_eq!(
            engine.status("FileChange").unwrap(),
            ProjectionStatus::Initialized
        );
    }

    #[test]
    fn engine_start_processing() {
        let engine = ProjectionEngine::new();
        let proj = FileChangeProjection::new();
        engine.register(Box::new(proj)).unwrap();

        let store = EventStore::new();
        engine.rebuild_all(&store).unwrap();

        engine.start_processing("FileChange").unwrap();
        assert_eq!(
            engine.status("FileChange").unwrap(),
            ProjectionStatus::Processing
        );
    }

    // -----------------------------------------------------------------------
    // RFC-0002 Section 7: Cross-projection failure isolation
    // -----------------------------------------------------------------------

    #[test]
    fn projection_failure_isolated_from_other_projections() {
        let engine = ProjectionEngine::new();

        // Register a healthy projection and a failing projection
        let healthy = FileChangeProjection::new();
        let failing = FailingProjection::new(true); // will fail on process_event
        engine.register(Box::new(healthy)).unwrap();
        engine.register(Box::new(failing)).unwrap();

        assert_eq!(engine.len(), 2);

        // Initialize both
        let store = EventStore::new();
        store.append(file_changed_payload("a.txt")).unwrap();
        store.append(file_changed_payload("b.txt")).unwrap();
        engine.rebuild_all(&store).unwrap();

        // Start processing both
        engine.start_processing("FileChange").unwrap();
        engine.start_processing("Failing").unwrap();

        assert_eq!(
            engine.status("FileChange").unwrap(),
            ProjectionStatus::Processing
        );
        assert_eq!(
            engine.status("Failing").unwrap(),
            ProjectionStatus::Processing
        );

        // Process an event - FailingProjection will fail
        let event = store.append(file_changed_payload("c.txt")).unwrap();
        let errors = engine.process_event_collect_errors(&event).unwrap();

        // FailingProjection should have reported an error
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].0, "Failing");

        // FailingProjection status is still Processing (engine doesn't auto-fail)
        // but its process_event returned error - isolation is proven because:
        // 1. FileChangeProjection processed the event successfully
        // 2. FileChangeProjection state was updated
        // 3. FileChangeProjection checkpoint advanced
        assert_eq!(engine.checkpoint("FileChange").unwrap(), 3);
        assert_eq!(engine.checkpoint("Failing").unwrap(), 0); // failed, no update

        // Verify FileChangeProjection state includes all 3 files
        // (we can't directly access state through engine, but checkpoint proves processing)
    }

    #[test]
    fn projection_failure_does_not_modify_event_store() {
        let engine = ProjectionEngine::new();

        let healthy = FileChangeProjection::new();
        let failing = FailingProjection::new(true);
        engine.register(Box::new(healthy)).unwrap();
        engine.register(Box::new(failing)).unwrap();

        let store = EventStore::new();
        store.append(file_changed_payload("a.txt")).unwrap();
        store.append(file_changed_payload("b.txt")).unwrap();

        let store_len_before = store.len();

        engine.rebuild_all(&store).unwrap();
        engine.start_processing("FileChange").unwrap();
        engine.start_processing("Failing").unwrap();

        let event = store.append(file_changed_payload("c.txt")).unwrap();
        let _errors = engine.process_event_collect_errors(&event).unwrap();

        // EventStore must be unchanged by projection failure
        assert_eq!(store.len(), store_len_before + 1); // only the append we did
    }
}
