use crate::event::{Event, EventPayload};
use crate::store::{EventStore, EventStoreError, ReplayResult};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Metadata (cache only — events.log is source of truth)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoreMeta {
    next_sequence: u64,
    event_count: u64,
}

// ---------------------------------------------------------------------------
// Verification result
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct VerificationResult {
    pub event_count: usize,
    pub errors: Vec<String>,
    pub valid: bool,
}

// ---------------------------------------------------------------------------
// Persistent Event Store
// ---------------------------------------------------------------------------

pub struct PersistentEventStore {
    log_path: PathBuf,
    meta_path: PathBuf,
    writer: Mutex<BufWriter<File>>,
    meta: Mutex<StoreMeta>,
    capacity: Option<usize>,
}

impl PersistentEventStore {
    /// Open or create a persistent event store.
    ///
    /// On startup:
    /// 1. Read meta.json (cache)
    /// 2. Scan events.log to verify sequence
    /// 3. Repair meta if mismatch
    pub fn open(dir: PathBuf) -> Result<Self, EventStoreError> {
        std::fs::create_dir_all(&dir)
            .map_err(|e| EventStoreError::StorageFailure(e.to_string()))?;

        let log_path = dir.join("events.log");
        let meta_path = dir.join("meta.json");

        // Read cache (if exists)
        let cached_meta = if meta_path.exists() {
            std::fs::read_to_string(&meta_path)
                .ok()
                .and_then(|s| serde_json::from_str::<StoreMeta>(&s).ok())
        } else {
            None
        };

        // Scan log to verify
        let log_meta = Self::scan_log(&log_path)?;

        // Resolve meta: prefer log, repair cache if mismatch
        let meta = match (cached_meta, log_meta) {
            (Some(cached), Some(log)) => {
                if cached.next_sequence == log.next_sequence {
                    cached
                } else {
                    // Log is truth, repair cache
                    log
                }
            }
            (None, Some(log)) => log,
            (Some(cached), None) => {
                // Empty log, trust cache if consistent
                if cached.event_count == 0 {
                    cached
                } else {
                    // Log cleared but cache has data — reset
                    StoreMeta {
                        next_sequence: 1,
                        event_count: 0,
                    }
                }
            }
            (None, None) => StoreMeta {
                next_sequence: 1,
                event_count: 0,
            },
        };

        // Persist repaired meta
        Self::write_meta_atomic(&meta_path, &meta)?;

        // Open log for appending
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| EventStoreError::StorageFailure(e.to_string()))?;

        Ok(Self {
            log_path,
            meta_path,
            writer: Mutex::new(BufWriter::new(file)),
            meta: Mutex::new(meta),
            capacity: None,
        })
    }

    /// Create a new persistent store with bounded capacity.
    pub fn with_capacity(dir: PathBuf, capacity: usize) -> Result<Self, EventStoreError> {
        let mut store = Self::open(dir)?;
        store.capacity = Some(capacity);
        Ok(store)
    }

    /// Scan events.log to determine actual sequence state.
    fn scan_log(log_path: &PathBuf) -> Result<Option<StoreMeta>, EventStoreError> {
        if !log_path.exists() {
            return Ok(None);
        }

        let file =
            File::open(log_path).map_err(|e| EventStoreError::ReplayFailure(e.to_string()))?;
        let reader = BufReader::new(file);

        let mut count = 0u64;
        let mut max_seq = 0u64;

        for line in reader.lines() {
            let line = line.map_err(|e| EventStoreError::ReplayFailure(e.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }

            let event: Event = serde_json::from_str(&line)
                .map_err(|e| EventStoreError::ReplayFailure(e.to_string()))?;

            if event.sequence > max_seq {
                max_seq = event.sequence;
            }
            count += 1;
        }

        Ok(Some(StoreMeta {
            next_sequence: max_seq + 1,
            event_count: count,
        }))
    }

    /// Write metadata atomically (tmp + rename).
    fn write_meta_atomic(path: &PathBuf, meta: &StoreMeta) -> Result<(), EventStoreError> {
        let tmp_path = path.with_extension("json.tmp");
        let data = serde_json::to_string_pretty(meta)
            .map_err(|e| EventStoreError::StorageFailure(e.to_string()))?;

        std::fs::write(&tmp_path, data)
            .map_err(|e| EventStoreError::StorageFailure(e.to_string()))?;

        // Atomic rename
        std::fs::rename(&tmp_path, path)
            .map_err(|e| EventStoreError::StorageFailure(e.to_string()))?;

        Ok(())
    }

    /// Verify the integrity of the event log.
    pub fn verify(&self) -> Result<VerificationResult, EventStoreError> {
        let file = File::open(&self.log_path)
            .map_err(|e| EventStoreError::ReplayFailure(e.to_string()))?;
        let reader = BufReader::new(file);

        let mut events = Vec::new();
        let mut errors = Vec::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| EventStoreError::ReplayFailure(e.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<Event>(&line) {
                Ok(event) => events.push(event),
                Err(e) => {
                    errors.push(format!("line {}: {}", line_num + 1, e));
                }
            }
        }

        // Check ordering
        for window in events.windows(2) {
            if window[0].sequence >= window[1].sequence {
                errors.push(format!(
                    "ordering corruption: {} >= {}",
                    window[0].sequence, window[1].sequence
                ));
            }
        }

        // Check gaps
        for (i, event) in events.iter().enumerate() {
            let expected = (i + 1) as u64;
            if event.sequence != expected {
                errors.push(format!(
                    "sequence gap at {}: expected {}, got {}",
                    i, expected, event.sequence
                ));
            }
        }

        Ok(VerificationResult {
            event_count: events.len(),
            valid: errors.is_empty(),
            errors,
        })
    }

    /// Return the path to the events log.
    pub fn log_path(&self) -> &PathBuf {
        &self.log_path
    }
}

impl EventStore for PersistentEventStore {
    fn append(&self, payload: EventPayload) -> Result<Event, EventStoreError> {
        let mut meta = self.meta.lock().map_err(|e| {
            EventStoreError::StorageFailure(format!("failed to acquire meta lock: {}", e))
        })?;

        // Backpressure check
        if let Some(cap) = self.capacity
            && meta.event_count >= cap as u64
        {
            return Err(EventStoreError::Backpressure);
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let event = Event {
            id: uuid::Uuid::new_v4(),
            sequence: meta.next_sequence,
            occurred_at: now,
            payload,
        };

        // Serialize to JSONL
        let mut line = serde_json::to_string(&event)
            .map_err(|e| EventStoreError::StorageFailure(e.to_string()))?;
        line.push('\n');

        // Write to log
        let mut writer = self.writer.lock().map_err(|e| {
            EventStoreError::StorageFailure(format!("failed to acquire writer lock: {}", e))
        })?;

        writer
            .write_all(line.as_bytes())
            .map_err(|e| EventStoreError::StorageFailure(e.to_string()))?;
        writer
            .flush()
            .map_err(|e| EventStoreError::StorageFailure(e.to_string()))?;

        // Update metadata
        meta.next_sequence += 1;
        meta.event_count += 1;

        // Persist metadata atomically
        Self::write_meta_atomic(&self.meta_path, &meta)?;

        Ok(event)
    }

    fn replay(&self, start_sequence: u64) -> Result<ReplayResult, EventStoreError> {
        let meta = self.meta.lock().map_err(|e| {
            EventStoreError::StorageFailure(format!("failed to acquire meta lock: {}", e))
        })?;

        if meta.event_count == 0 {
            return Ok(ReplayResult {
                events: Vec::new(),
                snapshot_end: 0,
            });
        }

        let snapshot_end = meta.next_sequence - 1;
        drop(meta);

        // Read from log
        let file = File::open(&self.log_path)
            .map_err(|e| EventStoreError::ReplayFailure(e.to_string()))?;
        let reader = BufReader::new(file);

        let mut events = Vec::new();
        for (line_num, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| EventStoreError::ReplayFailure(e.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }

            let event: Event = serde_json::from_str(&line).map_err(|e| {
                EventStoreError::ReplayFailure(format!("line {}: {}", line_num + 1, e))
            })?;

            if event.sequence >= start_sequence && event.sequence <= snapshot_end {
                events.push(event);
            }
        }

        // Verify ordering
        for window in events.windows(2) {
            if window[0].sequence >= window[1].sequence {
                return Err(EventStoreError::ReplayFailure(format!(
                    "ordering corruption: {} >= {}",
                    window[0].sequence, window[1].sequence
                )));
            }
        }

        // Verify no gaps
        for (i, event) in events.iter().enumerate() {
            let expected = start_sequence + i as u64;
            if event.sequence != expected {
                return Err(EventStoreError::ReplayFailure(format!(
                    "sequence gap: expected {}, got {}",
                    expected, event.sequence
                )));
            }
        }

        Ok(ReplayResult {
            events,
            snapshot_end,
        })
    }

    fn len(&self) -> usize {
        self.meta
            .lock()
            .map(|m| m.event_count as usize)
            .unwrap_or(0)
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn next_sequence(&self) -> u64 {
        self.meta.lock().map(|m| m.next_sequence).unwrap_or(1)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{FileChanged, FileOperation};
    use crate::store::EventStore;
    use tempfile::TempDir;

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn temp_store() -> (PersistentEventStore, TempDir) {
        let dir = TempDir::new().unwrap();
        let store = PersistentEventStore::open(dir.path().to_path_buf()).unwrap();
        (store, dir)
    }

    fn file_changed_payload(path: &str) -> EventPayload {
        EventPayload::FileChanged(FileChanged {
            path: std::path::PathBuf::from(path),
            operation: FileOperation::Created,
            timestamp: now(),
        })
    }

    #[test]
    fn append_persists_to_disk() {
        let (store, dir) = temp_store();
        let event = store.append(file_changed_payload("a.txt")).unwrap();

        // Reopen store
        let store2 = PersistentEventStore::open(dir.path().to_path_buf()).unwrap();
        let result = store2.replay(1).unwrap();

        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].id, event.id);
    }

    #[test]
    fn replay_survives_restart() {
        let (store, dir) = temp_store();
        store.append(file_changed_payload("a.txt")).unwrap();
        store.append(file_changed_payload("b.txt")).unwrap();

        // Reopen
        let store2 = PersistentEventStore::open(dir.path().to_path_buf()).unwrap();
        let result = store2.replay(1).unwrap();

        assert_eq!(result.events.len(), 2);
    }

    #[test]
    fn sequence_continues_after_restart() {
        let (store, dir) = temp_store();
        let e1 = store.append(file_changed_payload("a.txt")).unwrap();
        let e2 = store.append(file_changed_payload("b.txt")).unwrap();
        let e3 = store.append(file_changed_payload("c.txt")).unwrap();

        assert_eq!(e1.sequence, 1);
        assert_eq!(e2.sequence, 2);
        assert_eq!(e3.sequence, 3);

        // Reopen
        let store2 = PersistentEventStore::open(dir.path().to_path_buf()).unwrap();
        assert_eq!(store2.next_sequence(), 4);

        let e4 = store2.append(file_changed_payload("d.txt")).unwrap();
        assert_eq!(e4.sequence, 4);
    }

    #[test]
    fn verify_detects_corruption() {
        let (store, dir) = temp_store();
        std::fs::write(dir.path().join("events.log"), "invalid json\n").unwrap();

        let result = store.verify().unwrap();
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn verify_detects_sequence_gap() {
        let (store, dir) = temp_store();

        // Manually write events with gap
        let mut meta = StoreMeta {
            next_sequence: 1,
            event_count: 0,
        };

        let mut log_file = File::create(dir.path().join("events.log")).unwrap();

        // Write event with sequence 1
        let event1 = Event {
            id: uuid::Uuid::new_v4(),
            sequence: 1,
            occurred_at: now(),
            payload: file_changed_payload("a.txt"),
        };
        writeln!(log_file, "{}", serde_json::to_string(&event1).unwrap()).unwrap();
        meta.next_sequence = 2;
        meta.event_count = 1;

        // Write event with sequence 3 (gap!)
        let event3 = Event {
            id: uuid::Uuid::new_v4(),
            sequence: 3,
            occurred_at: now(),
            payload: file_changed_payload("c.txt"),
        };
        writeln!(log_file, "{}", serde_json::to_string(&event3).unwrap()).unwrap();
        meta.next_sequence = 4;
        meta.event_count = 2;

        // Write meta with the gap
        let meta_json = serde_json::to_string_pretty(&meta).unwrap();
        std::fs::write(dir.path().join("meta.json"), meta_json).unwrap();

        // Verify should detect gap
        let result = store.verify().unwrap();
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("sequence gap")));
    }

    #[test]
    fn crash_recovery_meta_repair() {
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("events.log");
        let meta_path = dir.path().join("meta.json");

        // Write events
        let store = PersistentEventStore::open(dir.path().to_path_buf()).unwrap();
        store.append(file_changed_payload("a.txt")).unwrap();
        store.append(file_changed_payload("b.txt")).unwrap();

        // Corrupt meta.json (simulate crash)
        std::fs::write(&meta_path, r#"{"next_sequence":999,"event_count":999}"#).unwrap();

        // Reopen — should repair
        let store2 = PersistentEventStore::open(dir.path().to_path_buf()).unwrap();
        assert_eq!(store2.next_sequence(), 3); // from log, not corrupted meta
        assert_eq!(store2.len(), 2);
    }

    #[test]
    fn verify_valid_store() {
        let (store, _dir) = temp_store();
        store.append(file_changed_payload("a.txt")).unwrap();
        store.append(file_changed_payload("b.txt")).unwrap();
        store.append(file_changed_payload("c.txt")).unwrap();

        let result = store.verify().unwrap();
        assert!(result.valid);
        assert_eq!(result.event_count, 3);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn backpressure_blocks_at_capacity() {
        let dir = TempDir::new().unwrap();
        let store = PersistentEventStore::with_capacity(dir.path().to_path_buf(), 2).unwrap();
        let _e1 = store.append(file_changed_payload("a.txt")).unwrap();
        let _e2 = store.append(file_changed_payload("b.txt")).unwrap();

        let result = store.append(file_changed_payload("c.txt"));
        assert!(matches!(result, Err(EventStoreError::Backpressure)));
    }

    #[test]
    fn empty_store_replay() {
        let (store, _dir) = temp_store();
        let result = store.replay(1).unwrap();
        assert!(result.events.is_empty());
        assert_eq!(result.snapshot_end, 0);
    }

    #[test]
    fn log_path_is_accessible() {
        let (store, dir) = temp_store();
        let expected = dir.path().join("events.log");
        assert_eq!(store.log_path(), &expected);
    }
}
