use std::path::PathBuf;
use std::time::SystemTime;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// File operations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileOperation {
    Created,
    Modified,
    Deleted,
}

// ---------------------------------------------------------------------------
// FileChanged event
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChanged {
    pub path: PathBuf,
    pub operation: FileOperation,
    pub timestamp: SystemTime,
}

// ---------------------------------------------------------------------------
// Event payload
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventPayload {
    FileChanged(FileChanged),
}

// ---------------------------------------------------------------------------
// Event envelope
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub id: Uuid,
    pub sequence: u64,
    pub occurred_at: SystemTime,
    pub payload: EventPayload,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_operation_equality() {
        assert_eq!(FileOperation::Created, FileOperation::Created);
        assert_ne!(FileOperation::Created, FileOperation::Modified);
    }

    #[test]
    fn event_payload_clone() {
        let payload = EventPayload::FileChanged(FileChanged {
            path: PathBuf::from("test.txt"),
            operation: FileOperation::Created,
            timestamp: SystemTime::now(),
        });
        let _cloned = payload.clone();
    }

    #[test]
    fn event_is_clone() {
        let event = Event {
            id: Uuid::new_v4(),
            sequence: 1,
            occurred_at: SystemTime::now(),
            payload: EventPayload::FileChanged(FileChanged {
                path: PathBuf::from("test.txt"),
                operation: FileOperation::Created,
                timestamp: SystemTime::now(),
            }),
        };
        let _cloned = event.clone();
    }
}
