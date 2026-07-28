use crate::event::{Event, EventPayload, FileOperation};
use crate::store::{EventStore, EventStoreError};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

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
}

impl FileChangeProjection {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(HashMap::new()),
        }
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

        Ok(())
    }

    /// Rebuild projection state from replay.
    pub fn rebuild(&self, store: &EventStore) -> Result<(), EventStoreError> {
        // Clear current state
        {
            let mut state = self.state.lock().map_err(|e| {
                EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
            })?;
            state.clear();
        }

        // Replay all events from sequence 0
        let result = store.replay(1)?;

        // Process each event
        for event in &result.events {
            self.process_event(event)?;
        }

        Ok(())
    }

    /// Get the current projection state.
    pub fn state(&self) -> HashMap<PathBuf, FileRecord> {
        self.state
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default()
    }

    /// Get the number of tracked files.
    pub fn file_count(&self) -> usize {
        self.state.lock().map(|s| s.len()).unwrap_or(0)
    }

    /// Get a specific file record.
    pub fn get_file(&self, path: &PathBuf) -> Option<FileRecord> {
        self.state.lock().unwrap().get(path).cloned()
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
}
