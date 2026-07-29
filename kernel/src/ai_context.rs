use crate::engine::Projection;
use crate::event::{Event, EventPayload, FileOperation};
use crate::projection::ProjectionStatus;
use crate::store::{EventStore, EventStoreError};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// AI Context Projection (RFC-0002, Section 9)
// ---------------------------------------------------------------------------

/// Summarizes system state for AI consumption.
/// Exposes semantic state, not event dumps.
#[derive(Debug, Clone)]
pub struct AiContextSummary {
    pub workspace_status: WorkspaceStatus,
    pub recent_changes: Vec<String>,
    pub file_count: usize,
    pub total_changes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceStatus {
    Empty,
    Active,
    Idle,
}

pub struct AiContextProjection {
    status: Mutex<ProjectionStatus>,
    checkpoint: Mutex<u64>,
    state: Mutex<AiContextState>,
}

struct AiContextState {
    files: HashMap<PathBuf, FileInfo>,
    recent_changes: Vec<String>,
}

#[derive(Debug, Clone)]
struct FileInfo {
    last_operation: FileOperation,
    change_count: u64,
}

impl AiContextProjection {
    pub fn new() -> Self {
        Self {
            status: Mutex::new(ProjectionStatus::Created),
            checkpoint: Mutex::new(0),
            state: Mutex::new(AiContextState {
                files: HashMap::new(),
                recent_changes: Vec::new(),
            }),
        }
    }

    /// Get AI context summary (semantic state for AI consumption).
    pub fn summary(&self) -> AiContextSummary {
        let state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let file_count = state.files.len();
        let total_changes: u64 = state.files.values().map(|f| f.change_count).sum();

        let workspace_status = if file_count == 0 {
            WorkspaceStatus::Empty
        } else if state.recent_changes.is_empty() {
            WorkspaceStatus::Idle
        } else {
            WorkspaceStatus::Active
        };

        AiContextSummary {
            workspace_status,
            recent_changes: state.recent_changes.clone(),
            file_count,
            total_changes,
        }
    }
}

impl Default for AiContextProjection {
    fn default() -> Self {
        Self::new()
    }
}

impl Projection for AiContextProjection {
    fn name(&self) -> &str {
        "AiContext"
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
        let mut state = self.state.lock().map_err(|e| {
            EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
        })?;

        if let EventPayload::FileChanged(fc) = &event.payload {
            let info = state
                .files
                .entry(fc.path.clone())
                .or_insert_with(|| FileInfo {
                    last_operation: fc.operation,
                    change_count: 0,
                });
            info.last_operation = fc.operation;
            info.change_count += 1;

            // Track recent changes (keep last 10)
            let change_desc = format!(
                "{} {}",
                fc.path.display(),
                format!("{:?}", fc.operation).to_lowercase()
            );
            state.recent_changes.push(change_desc);
            if state.recent_changes.len() > 10 {
                state.recent_changes.remove(0);
            }
        }

        // Update checkpoint
        let mut cp = self.checkpoint.lock().map_err(|e| {
            EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
        })?;
        *cp = event.sequence;

        Ok(())
    }

    fn rebuild(&self, store: &EventStore) -> Result<(), EventStoreError> {
        // Clear state
        {
            let mut state = self.state.lock().map_err(|e| {
                EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
            })?;
            state.files.clear();
            state.recent_changes.clear();
        }

        // Reset checkpoint
        {
            let mut cp = self.checkpoint.lock().map_err(|e| {
                EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
            })?;
            *cp = 0;
        }

        // Replay all events
        let result = store.replay(1)?;
        for event in &result.events {
            self.process_event(event)?;
        }

        Ok(())
    }

    fn initialize(&self, store: &EventStore) -> Result<(), EventStoreError> {
        self.rebuild(store)?;
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
        if !status.can_transition_to(ProjectionStatus::Failed) {
            return Err(EventStoreError::ProjectionFailure(format!(
                "invalid transition: {:?} -> Failed",
                *status
            )));
        }
        *status = ProjectionStatus::Failed;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EventPayload, FileChanged};
    use crate::store::EventStore;
    use std::time::SystemTime;

    fn file_changed_payload(path: &str) -> EventPayload {
        EventPayload::FileChanged(FileChanged {
            path: PathBuf::from(path),
            operation: FileOperation::Created,
            timestamp: SystemTime::now(),
        })
    }

    #[test]
    fn ai_context_starts_empty() {
        let proj = AiContextProjection::new();
        let summary = proj.summary();
        assert_eq!(summary.workspace_status, WorkspaceStatus::Empty);
        assert_eq!(summary.file_count, 0);
        assert_eq!(summary.total_changes, 0);
    }

    #[test]
    fn ai_context_summary_after_rebuild() {
        let proj = AiContextProjection::new();
        let store = EventStore::new();

        store.append(file_changed_payload("a.txt")).unwrap();
        store.append(file_changed_payload("b.txt")).unwrap();
        store.append(file_changed_payload("a.txt")).unwrap();

        proj.initialize(&store).unwrap();

        let summary = proj.summary();
        assert_eq!(summary.workspace_status, WorkspaceStatus::Active);
        assert_eq!(summary.file_count, 2);
        assert_eq!(summary.total_changes, 3);
    }

    #[test]
    fn ai_context_recent_changes_tracked() {
        let proj = AiContextProjection::new();
        let store = EventStore::new();

        for i in 0..15 {
            store
                .append(file_changed_payload(&format!("file_{}.txt", i)))
                .unwrap();
        }

        proj.initialize(&store).unwrap();

        let summary = proj.summary();
        assert_eq!(summary.recent_changes.len(), 10); // capped at 10
    }

    #[test]
    fn ai_context_is_projection() {
        let proj = AiContextProjection::new();
        assert_eq!(proj.name(), "AiContext");
        assert_eq!(proj.status(), ProjectionStatus::Created);
        assert_eq!(proj.checkpoint(), 0);
    }
}
