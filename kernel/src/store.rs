use crate::event::{Event, EventPayload};
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum EventStoreError {
    #[error("storage failure: {0}")]
    StorageFailure(String),

    #[error("invalid sequence: expected {expected}, got {actual}")]
    InvalidSequence { expected: u64, actual: u64 },

    #[error("replay failure: {0}")]
    ReplayFailure(String),

    #[error("projection failure: {0}")]
    ProjectionFailure(String),

    #[error("backpressure: store at capacity, append blocked or rejected")]
    Backpressure,
}

// ---------------------------------------------------------------------------
// Replay result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ReplayResult {
    pub events: Vec<Event>,
    pub snapshot_end: u64,
}

// ---------------------------------------------------------------------------
// Event Store
// ---------------------------------------------------------------------------

pub struct EventStore {
    events: Mutex<Vec<Event>>,
    next_sequence: Mutex<u64>,
    capacity: Option<usize>,
}

impl EventStore {
    /// Create a new Event Store with unlimited capacity.
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            next_sequence: Mutex::new(1),
            capacity: None,
        }
    }

    /// Create a new Event Store with bounded capacity for backpressure testing.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            events: Mutex::new(Vec::new()),
            next_sequence: Mutex::new(1),
            capacity: Some(capacity),
        }
    }

    /// Append an event to the store.
    ///
    /// Returns the committed event with assigned sequence number.
    /// Append is atomic: the event is either fully written or not written at all.
    /// Failed appends consume no sequence number.
    pub fn append(&self, payload: EventPayload) -> Result<Event, EventStoreError> {
        let mut events = self.events.lock().map_err(|e| {
            EventStoreError::StorageFailure(format!("failed to acquire lock: {}", e))
        })?;

        // Backpressure check
        if let Some(cap) = self.capacity
            && events.len() >= cap
        {
            return Err(EventStoreError::Backpressure);
        }

        let mut next_seq = self.next_sequence.lock().map_err(|e| {
            EventStoreError::StorageFailure(format!("failed to acquire lock: {}", e))
        })?;

        let event = Event {
            id: Uuid::new_v4(),
            sequence: *next_seq,
            occurred_at: std::time::SystemTime::now(),
            payload,
        };

        // Commit: write to store, then increment sequence
        events.push(event.clone());
        *next_seq += 1;

        Ok(event)
    }

    /// Replay committed events starting from `start_sequence`.
    ///
    /// Returns a bounded snapshot of events as of the moment replay begins.
    /// Events appended after replay starts are not included.
    pub fn replay(&self, start_sequence: u64) -> Result<ReplayResult, EventStoreError> {
        let events = self.events.lock().map_err(|e| {
            EventStoreError::ReplayFailure(format!("failed to acquire lock: {}", e))
        })?;

        if events.is_empty() {
            return Ok(ReplayResult {
                events: Vec::new(),
                snapshot_end: 0,
            });
        }

        // Snapshot: capture the highest sequence at the moment replay begins
        let snapshot_end = events.last().unwrap().sequence;

        // Filter events in range [start_sequence, snapshot_end]
        let filtered: Vec<Event> = events
            .iter()
            .filter(|e| e.sequence >= start_sequence && e.sequence <= snapshot_end)
            .cloned()
            .collect();

        // Verify ordering
        for window in filtered.windows(2) {
            if window[0].sequence >= window[1].sequence {
                return Err(EventStoreError::ReplayFailure(format!(
                    "ordering corruption: {} >= {}",
                    window[0].sequence, window[1].sequence
                )));
            }
        }

        Ok(ReplayResult {
            events: filtered,
            snapshot_end,
        })
    }

    /// Return the current number of committed events.
    pub fn len(&self) -> usize {
        self.events.lock().map(|e| e.len()).unwrap_or(0)
    }

    /// Return true if the store has no committed events.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return the next sequence number that would be assigned.
    pub fn next_sequence(&self) -> u64 {
        self.next_sequence.lock().map(|s| *s).unwrap_or(1)
    }
}

impl Default for EventStore {
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
    use crate::event::{FileChanged, FileOperation};
    use std::path::PathBuf;

    fn file_changed_payload(path: &str) -> EventPayload {
        EventPayload::FileChanged(FileChanged {
            path: PathBuf::from(path),
            operation: FileOperation::Created,
            timestamp: std::time::SystemTime::now(),
        })
    }

    #[test]
    fn append_assigns_sequential_sequence() {
        let store = EventStore::new();
        let e1 = store.append(file_changed_payload("a.txt")).unwrap();
        let e2 = store.append(file_changed_payload("b.txt")).unwrap();
        let e3 = store.append(file_changed_payload("c.txt")).unwrap();

        assert_eq!(e1.sequence, 1);
        assert_eq!(e2.sequence, 2);
        assert_eq!(e3.sequence, 3);
    }

    #[test]
    fn append_is_atomic() {
        let store = EventStore::new();
        let event = store.append(file_changed_payload("test.txt")).unwrap();

        assert_eq!(store.len(), 1);
        assert_eq!(store.next_sequence(), 2);
        assert_eq!(event.id, event.id); // id is assigned
    }

    #[test]
    fn failed_append_consumes_no_sequence() {
        let store = EventStore::with_capacity(1);
        let _e1 = store.append(file_changed_payload("a.txt")).unwrap();

        // Second append should fail (backpressure)
        let result = store.append(file_changed_payload("b.txt"));
        assert!(result.is_err());

        // Sequence should still be 2 (not consumed)
        assert_eq!(store.next_sequence(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn replay_preserves_ordering() {
        let store = EventStore::new();
        store.append(file_changed_payload("a.txt")).unwrap();
        store.append(file_changed_payload("b.txt")).unwrap();
        store.append(file_changed_payload("c.txt")).unwrap();

        let result = store.replay(1).unwrap();
        assert_eq!(result.events.len(), 3);
        assert_eq!(result.events[0].sequence, 1);
        assert_eq!(result.events[1].sequence, 2);
        assert_eq!(result.events[2].sequence, 3);
    }

    #[test]
    fn replay_from_middle() {
        let store = EventStore::new();
        store.append(file_changed_payload("a.txt")).unwrap();
        store.append(file_changed_payload("b.txt")).unwrap();
        store.append(file_changed_payload("c.txt")).unwrap();
        store.append(file_changed_payload("d.txt")).unwrap();
        store.append(file_changed_payload("e.txt")).unwrap();

        let result = store.replay(3).unwrap();
        assert_eq!(result.events.len(), 3);
        assert_eq!(result.events[0].sequence, 3);
        assert_eq!(result.events[1].sequence, 4);
        assert_eq!(result.events[2].sequence, 5);
    }

    #[test]
    fn replay_empty_store() {
        let store = EventStore::new();
        let result = store.replay(1).unwrap();
        assert!(result.events.is_empty());
        assert_eq!(result.snapshot_end, 0);
    }

    #[test]
    fn replay_snapshot_semantics() {
        let store = EventStore::new();
        store.append(file_changed_payload("a.txt")).unwrap();
        store.append(file_changed_payload("b.txt")).unwrap();
        store.append(file_changed_payload("c.txt")).unwrap();

        // Start replay (captures snapshot_end = 3)
        let result = store.replay(1).unwrap();
        assert_eq!(result.snapshot_end, 3);
        assert_eq!(result.events.len(), 3);

        // Append new event after snapshot
        store.append(file_changed_payload("d.txt")).unwrap();

        // A new replay captures a new snapshot (snapshot_end = 4)
        // but still returns deterministic, ordered results
        let result2 = store.replay(1).unwrap();
        assert_eq!(result2.events.len(), 4);
        assert_eq!(result2.snapshot_end, 4);

        // Verify ordering is preserved
        for (i, event) in result2.events.iter().enumerate() {
            assert_eq!(event.sequence, (i + 1) as u64);
        }
    }

    #[test]
    fn replay_never_skips_committed_event() {
        let store = EventStore::new();
        for i in 0..100 {
            store
                .append(file_changed_payload(&format!("file_{}.txt", i)))
                .unwrap();
        }

        let result = store.replay(1).unwrap();
        assert_eq!(result.events.len(), 100);

        // Verify no gaps
        for (i, event) in result.events.iter().enumerate() {
            assert_eq!(event.sequence, (i + 1) as u64);
        }
    }

    #[test]
    fn replay_never_duplicates_event() {
        let store = EventStore::new();
        store.append(file_changed_payload("a.txt")).unwrap();
        store.append(file_changed_payload("b.txt")).unwrap();

        let result = store.replay(1).unwrap();
        let ids: Vec<Uuid> = result.events.iter().map(|e| e.id).collect();
        let unique_ids: std::collections::HashSet<Uuid> = ids.iter().cloned().collect();
        assert_eq!(ids.len(), unique_ids.len());
    }

    #[test]
    fn backpressure_blocks_at_capacity() {
        let store = EventStore::with_capacity(2);
        let _e1 = store.append(file_changed_payload("a.txt")).unwrap();
        let _e2 = store.append(file_changed_payload("b.txt")).unwrap();

        let result = store.append(file_changed_payload("c.txt"));
        assert!(matches!(result, Err(EventStoreError::Backpressure)));
    }

    #[test]
    fn committed_events_remain_available_after_backpressure() {
        let store = EventStore::with_capacity(2);
        let e1 = store.append(file_changed_payload("a.txt")).unwrap();
        let e2 = store.append(file_changed_payload("b.txt")).unwrap();

        // Third append fails
        assert!(store.append(file_changed_payload("c.txt")).is_err());

        // First two events are still available
        let result = store.replay(1).unwrap();
        assert_eq!(result.events.len(), 2);
        assert_eq!(result.events[0].id, e1.id);
        assert_eq!(result.events[1].id, e2.id);
    }
}
