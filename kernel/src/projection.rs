use crate::event::{Event, EventPayload, FileOperation};
use crate::store::{EventStore, EventStoreError};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Projection lifecycle (RFC-0002, Section 5)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionStatus {
    Created,
    Initialized,
    Processing,
    Failed,
    Rebuilding,
}

impl ProjectionStatus {
    /// Check if transition from self to next is valid.
    pub fn can_transition_to(self, next: ProjectionStatus) -> bool {
        matches!(
            (self, next),
            (ProjectionStatus::Created, ProjectionStatus::Initialized)
                | (ProjectionStatus::Initialized, ProjectionStatus::Processing)
                | (ProjectionStatus::Initialized, ProjectionStatus::Rebuilding)
                | (ProjectionStatus::Processing, ProjectionStatus::Failed)
                | (ProjectionStatus::Processing, ProjectionStatus::Rebuilding)
                | (ProjectionStatus::Failed, ProjectionStatus::Rebuilding)
                | (ProjectionStatus::Rebuilding, ProjectionStatus::Initialized)
        )
    }
}

// ---------------------------------------------------------------------------
// Projection state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRecord {
    pub path: PathBuf,
    pub last_operation: FileOperation,
    pub change_count: u64,
}

// ---------------------------------------------------------------------------
// FileChangeProjection
// ---------------------------------------------------------------------------

pub struct FileChangeProjection {
    state: Mutex<HashMap<PathBuf, FileRecord>>,
    status: Mutex<ProjectionStatus>,
    checkpoint: Mutex<u64>,
}

impl FileChangeProjection {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(HashMap::new()),
            status: Mutex::new(ProjectionStatus::Created),
            checkpoint: Mutex::new(0),
        }
    }

    /// Return current projection status.
    pub fn status(&self) -> ProjectionStatus {
        self.status
            .lock()
            .map(|s| *s)
            .unwrap_or(ProjectionStatus::Failed)
    }

    /// Return current checkpoint offset.
    pub fn checkpoint(&self) -> u64 {
        self.checkpoint.lock().map(|c| *c).unwrap_or(0)
    }

    /// Transition projection to a new status. Returns error on invalid transition.
    fn set_status(&self, next: ProjectionStatus) -> Result<(), EventStoreError> {
        let mut status = self.status.lock().map_err(|e| {
            EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
        })?;
        if !status.can_transition_to(next) {
            return Err(EventStoreError::ProjectionFailure(format!(
                "invalid transition: {:?} -> {:?}",
                *status, next
            )));
        }
        *status = next;
        Ok(())
    }

    /// Process a single event. Only FileChanged events are relevant.
    pub fn process_event(&self, event: &Event) -> Result<(), EventStoreError> {
        let mut state = self.state.lock().map_err(|e| {
            EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
        })?;

        match &event.payload {
            EventPayload::FileChanged(fc) => {
                let entry = state.entry(fc.path.clone()).or_insert_with(|| FileRecord {
                    path: fc.path.clone(),
                    last_operation: fc.operation,
                    change_count: 0,
                });
                entry.last_operation = fc.operation;
                entry.change_count += 1;
            }
        }

        // Update checkpoint
        let mut cp = self.checkpoint.lock().map_err(|e| {
            EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
        })?;
        *cp = event.sequence;

        Ok(())
    }

    /// Rebuild projection state from replay.
    pub fn rebuild(&self, store: &EventStore) -> Result<(), EventStoreError> {
        // Only transition to Rebuilding if not in Created state
        {
            let current = self.status.lock().map_err(|e| {
                EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
            })?;
            if *current != ProjectionStatus::Created {
                drop(current);
                self.set_status(ProjectionStatus::Rebuilding)?;
            }
        }

        // Clear current state
        {
            let mut state = self.state.lock().map_err(|e| {
                EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
            })?;
            state.clear();
        }

        // Reset checkpoint
        {
            let mut cp = self.checkpoint.lock().map_err(|e| {
                EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
            })?;
            *cp = 0;
        }

        // Replay all events from sequence 0
        let result = store.replay(1)?;

        // Process each event
        for event in &result.events {
            self.process_event(event)?;
        }

        self.set_status(ProjectionStatus::Initialized)?;
        Ok(())
    }

    /// Initialize projection (Created -> Initialized).
    pub fn initialize(&self, store: &EventStore) -> Result<(), EventStoreError> {
        self.rebuild(store)
    }

    /// Mark projection as processing (Initialized -> Processing).
    pub fn start_processing(&self) -> Result<(), EventStoreError> {
        self.set_status(ProjectionStatus::Processing)
    }

    /// Mark projection as failed (Processing -> Failed).
    pub fn mark_failed(&self) -> Result<(), EventStoreError> {
        self.set_status(ProjectionStatus::Failed)
    }

    /// Get the current projection state.
    pub fn state(&self) -> HashMap<PathBuf, FileRecord> {
        self.state.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Get the number of tracked files.
    pub fn file_count(&self) -> usize {
        self.state.lock().map(|s| s.len()).unwrap_or(0)
    }

    /// Get a specific file record.
    pub fn get_file(&self, path: &PathBuf) -> Option<FileRecord> {
        self.state.lock().ok().and_then(|s| s.get(path).cloned())
    }
}

impl Default for FileChangeProjection {
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
    use crate::event::{Event, EventPayload, FileChanged, FileOperation};
    use crate::store::EventStore;
    use std::path::PathBuf;
    use std::time::SystemTime;
    use uuid::Uuid;

    fn test_event(path: &str, op: FileOperation) -> Event {
        Event {
            id: Uuid::new_v4(),
            sequence: 1,
            occurred_at: SystemTime::now(),
            payload: EventPayload::FileChanged(FileChanged {
                path: PathBuf::from(path),
                operation: op,
                timestamp: SystemTime::now(),
            }),
        }
    }

    #[test]
    fn process_file_created() {
        let proj = FileChangeProjection::new();
        let event = test_event("test.txt", FileOperation::Created);
        proj.process_event(&event).unwrap();

        let state = proj.state();
        assert_eq!(state.len(), 1);

        let record = state.get(&PathBuf::from("test.txt")).unwrap();
        assert_eq!(record.last_operation, FileOperation::Created);
        assert_eq!(record.change_count, 1);
    }

    #[test]
    fn process_file_modified() {
        let proj = FileChangeProjection::new();

        let e1 = test_event("test.txt", FileOperation::Created);
        proj.process_event(&e1).unwrap();

        let e2 = test_event("test.txt", FileOperation::Modified);
        proj.process_event(&e2).unwrap();

        let record = proj.get_file(&PathBuf::from("test.txt")).unwrap();
        assert_eq!(record.last_operation, FileOperation::Modified);
        assert_eq!(record.change_count, 2);
    }

    #[test]
    fn process_multiple_files() {
        let proj = FileChangeProjection::new();

        proj.process_event(&test_event("a.txt", FileOperation::Created))
            .unwrap();
        proj.process_event(&test_event("b.txt", FileOperation::Created))
            .unwrap();
        proj.process_event(&test_event("c.txt", FileOperation::Created))
            .unwrap();

        assert_eq!(proj.file_count(), 3);
    }

    #[test]
    fn rebuild_from_store() {
        let store = EventStore::new();
        let proj = FileChangeProjection::new();

        // Append events
        store
            .append(EventPayload::FileChanged(FileChanged {
                path: PathBuf::from("a.txt"),
                operation: FileOperation::Created,
                timestamp: SystemTime::now(),
            }))
            .unwrap();
        store
            .append(EventPayload::FileChanged(FileChanged {
                path: PathBuf::from("b.txt"),
                operation: FileOperation::Created,
                timestamp: SystemTime::now(),
            }))
            .unwrap();
        store
            .append(EventPayload::FileChanged(FileChanged {
                path: PathBuf::from("a.txt"),
                operation: FileOperation::Modified,
                timestamp: SystemTime::now(),
            }))
            .unwrap();

        // Rebuild projection
        proj.rebuild(&store).unwrap();

        // Verify state
        assert_eq!(proj.file_count(), 2);

        let a_record = proj.get_file(&PathBuf::from("a.txt")).unwrap();
        assert_eq!(a_record.change_count, 2);
        assert_eq!(a_record.last_operation, FileOperation::Modified);

        let b_record = proj.get_file(&PathBuf::from("b.txt")).unwrap();
        assert_eq!(b_record.change_count, 1);
        assert_eq!(b_record.last_operation, FileOperation::Created);
    }

    #[test]
    fn rebuild_is_idempotent() {
        let store = EventStore::new();
        let proj = FileChangeProjection::new();

        store
            .append(EventPayload::FileChanged(FileChanged {
                path: PathBuf::from("a.txt"),
                operation: FileOperation::Created,
                timestamp: SystemTime::now(),
            }))
            .unwrap();

        // Rebuild twice
        proj.rebuild(&store).unwrap();
        let state1 = proj.state();

        proj.rebuild(&store).unwrap();
        let state2 = proj.state();

        assert_eq!(state1, state2);
    }

    #[test]
    fn projection_state_is_disposable() {
        let store = EventStore::new();
        let proj = FileChangeProjection::new();

        store
            .append(EventPayload::FileChanged(FileChanged {
                path: PathBuf::from("a.txt"),
                operation: FileOperation::Created,
                timestamp: SystemTime::now(),
            }))
            .unwrap();
        store
            .append(EventPayload::FileChanged(FileChanged {
                path: PathBuf::from("b.txt"),
                operation: FileOperation::Modified,
                timestamp: SystemTime::now(),
            }))
            .unwrap();

        // Build projection
        proj.rebuild(&store).unwrap();
        let state_before = proj.state();

        // Clear and rebuild
        {
            let mut state = proj.state.lock().unwrap();
            state.clear();
        }
        proj.rebuild(&store).unwrap();
        let state_after = proj.state();

        // States must be identical
        assert_eq!(state_before, state_after);
    }

    #[test]
    fn projection_does_not_modify_store() {
        let store = EventStore::new();
        let proj = FileChangeProjection::new();

        store
            .append(EventPayload::FileChanged(FileChanged {
                path: PathBuf::from("a.txt"),
                operation: FileOperation::Created,
                timestamp: SystemTime::now(),
            }))
            .unwrap();

        let store_len_before = store.len();
        proj.rebuild(&store).unwrap();
        let store_len_after = store.len();

        assert_eq!(store_len_before, store_len_after);
    }

    // -----------------------------------------------------------------------
    // Lifecycle tests (RFC-0002, Section 5)
    // -----------------------------------------------------------------------

    #[test]
    fn lifecycle_created_to_initialized() {
        let proj = FileChangeProjection::new();
        assert_eq!(proj.status(), ProjectionStatus::Created);

        proj.initialize(&EventStore::new()).unwrap();
        assert_eq!(proj.status(), ProjectionStatus::Initialized);
    }

    #[test]
    fn lifecycle_initialized_to_processing() {
        let proj = FileChangeProjection::new();
        proj.initialize(&EventStore::new()).unwrap();

        proj.start_processing().unwrap();
        assert_eq!(proj.status(), ProjectionStatus::Processing);
    }

    #[test]
    fn lifecycle_processing_to_failed() {
        let proj = FileChangeProjection::new();
        proj.initialize(&EventStore::new()).unwrap();
        proj.start_processing().unwrap();

        proj.mark_failed().unwrap();
        assert_eq!(proj.status(), ProjectionStatus::Failed);
    }

    #[test]
    fn lifecycle_failed_to_rebuilding() {
        let proj = FileChangeProjection::new();
        proj.initialize(&EventStore::new()).unwrap();
        proj.start_processing().unwrap();
        proj.mark_failed().unwrap();

        proj.rebuild(&EventStore::new()).unwrap();
        assert_eq!(proj.status(), ProjectionStatus::Initialized);
    }

    #[test]
    fn lifecycle_processing_to_rebuilding() {
        let proj = FileChangeProjection::new();
        proj.initialize(&EventStore::new()).unwrap();
        proj.start_processing().unwrap();

        proj.rebuild(&EventStore::new()).unwrap();
        assert_eq!(proj.status(), ProjectionStatus::Initialized);
    }

    #[test]
    fn lifecycle_invalid_created_to_processing() {
        let proj = FileChangeProjection::new();
        let result = proj.start_processing();
        assert!(result.is_err());
    }

    #[test]
    fn lifecycle_invalid_failed_to_processing() {
        let proj = FileChangeProjection::new();
        proj.initialize(&EventStore::new()).unwrap();
        proj.start_processing().unwrap();
        proj.mark_failed().unwrap();

        let result = proj.start_processing();
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Checkpoint tests (RFC-0002, Section 10)
    // -----------------------------------------------------------------------

    #[test]
    fn checkpoint_starts_at_zero() {
        let proj = FileChangeProjection::new();
        assert_eq!(proj.checkpoint(), 0);
    }

    #[test]
    fn checkpoint_advances_on_process() {
        let store = EventStore::new();
        let proj = FileChangeProjection::new();

        let e1 = store
            .append(EventPayload::FileChanged(FileChanged {
                path: PathBuf::from("a.txt"),
                operation: FileOperation::Created,
                timestamp: SystemTime::now(),
            }))
            .unwrap();
        let e2 = store
            .append(EventPayload::FileChanged(FileChanged {
                path: PathBuf::from("b.txt"),
                operation: FileOperation::Created,
                timestamp: SystemTime::now(),
            }))
            .unwrap();

        proj.process_event(&e1).unwrap();
        assert_eq!(proj.checkpoint(), 1);

        proj.process_event(&e2).unwrap();
        assert_eq!(proj.checkpoint(), 2);
    }

    #[test]
    fn checkpoint_resets_on_rebuild() {
        let store = EventStore::new();
        let proj = FileChangeProjection::new();

        store
            .append(EventPayload::FileChanged(FileChanged {
                path: PathBuf::from("a.txt"),
                operation: FileOperation::Created,
                timestamp: SystemTime::now(),
            }))
            .unwrap();

        // Process one event manually
        let events = store.replay(1).unwrap();
        proj.process_event(&events.events[0]).unwrap();
        assert_eq!(proj.checkpoint(), 1);

        // Rebuild resets checkpoint to 1 (only 1 event in store)
        proj.rebuild(&store).unwrap();
        assert_eq!(proj.checkpoint(), 1);
    }

    #[test]
    fn rebuild_sets_checkpoint_to_last_event() {
        let store = EventStore::new();
        let proj = FileChangeProjection::new();

        for i in 0..5 {
            store
                .append(EventPayload::FileChanged(FileChanged {
                    path: PathBuf::from(format!("file_{}.txt", i)),
                    operation: FileOperation::Created,
                    timestamp: SystemTime::now(),
                }))
                .unwrap();
        }

        proj.rebuild(&store).unwrap();
        assert_eq!(proj.checkpoint(), 5);
    }

    // -----------------------------------------------------------------------
    // RFC-0002 Section 10: Checkpoint associativity
    // -----------------------------------------------------------------------

    #[test]
    fn checkpoint_associativity_full_rebuild_equals_resume() {
        // Create 5 events
        let store = EventStore::new();
        let ops = [
            (PathBuf::from("a.txt"), FileOperation::Created),
            (PathBuf::from("a.txt"), FileOperation::Modified),
            (PathBuf::from("b.txt"), FileOperation::Created),
            (PathBuf::from("a.txt"), FileOperation::Deleted),
            (PathBuf::from("c.txt"), FileOperation::Created),
        ];

        for (path, op) in &ops {
            store
                .append(EventPayload::FileChanged(FileChanged {
                    path: path.clone(),
                    operation: *op,
                    timestamp: SystemTime::now(),
                }))
                .unwrap();
        }

        // Path A: Full rebuild from sequence 0
        let full_proj = FileChangeProjection::new();
        full_proj.rebuild(&store).unwrap();
        let full_state = full_proj.state();
        let full_checkpoint = full_proj.checkpoint();

        // Path B: Process events 1..3, then 4..5 (two batches)
        let batch_proj = FileChangeProjection::new();
        let events = store.replay(1).unwrap();

        // Batch 1: events 1..3
        for event in events.events.iter().take(3) {
            batch_proj.process_event(event).unwrap();
        }
        assert_eq!(batch_proj.checkpoint(), 3);

        // Batch 2: events 4..5
        for event in events.events.iter().skip(3) {
            batch_proj.process_event(event).unwrap();
        }

        // Verify: batch checkpoint == full checkpoint
        assert_eq!(batch_proj.checkpoint(), full_checkpoint);
        assert_eq!(batch_proj.checkpoint(), 5);

        // Verify: batch state == full rebuild state
        let batch_state = batch_proj.state();
        assert_eq!(full_state, batch_state);

        // Verify specific state contents
        let a_full = full_state.get(&PathBuf::from("a.txt")).unwrap();
        let a_batch = batch_state.get(&PathBuf::from("a.txt")).unwrap();
        assert_eq!(a_full.change_count, 3); // Created, Modified, Deleted
        assert_eq!(a_full.last_operation, FileOperation::Deleted);
        assert_eq!(a_batch.change_count, 3);
        assert_eq!(a_batch.last_operation, FileOperation::Deleted);

        assert!(full_state.contains_key(&PathBuf::from("b.txt")));
        assert!(full_state.contains_key(&PathBuf::from("c.txt")));
    }
}
