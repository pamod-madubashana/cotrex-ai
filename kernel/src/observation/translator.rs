use crate::event::{EventPayload, FileChanged, FileOperation};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Raw Observation (RFC-0003, Section 2)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawObservation {
    pub path: PathBuf,
    pub operation: RawOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawOperation {
    Created,
    Modified,
    Deleted,
    Renamed { from: PathBuf },
}

// ---------------------------------------------------------------------------
// Translation Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationError {
    pub reason: String,
    pub observation: RawObservation,
}

// ---------------------------------------------------------------------------
// Translator (RFC-0003, Section 5)
// ---------------------------------------------------------------------------

pub struct Translator;

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

impl Translator {
    /// Translate a raw observation into a FileChanged event payload.
    ///
    /// Returns one or two events for renames (delete + create).
    pub fn translate(observation: &RawObservation) -> Result<Vec<EventPayload>, TranslationError> {
        let mut events = Vec::new();

        match observation.operation {
            RawOperation::Created => {
                events.push(EventPayload::FileChanged(FileChanged {
                    path: observation.path.clone(),
                    operation: FileOperation::Created,
                    timestamp: now(),
                }));
            }
            RawOperation::Modified => {
                events.push(EventPayload::FileChanged(FileChanged {
                    path: observation.path.clone(),
                    operation: FileOperation::Modified,
                    timestamp: now(),
                }));
            }
            RawOperation::Deleted => {
                events.push(EventPayload::FileChanged(FileChanged {
                    path: observation.path.clone(),
                    operation: FileOperation::Deleted,
                    timestamp: now(),
                }));
            }
            RawOperation::Renamed { ref from } => {
                events.push(EventPayload::FileChanged(FileChanged {
                    path: from.clone(),
                    operation: FileOperation::Deleted,
                    timestamp: now(),
                }));
                events.push(EventPayload::FileChanged(FileChanged {
                    path: observation.path.clone(),
                    operation: FileOperation::Created,
                    timestamp: now(),
                }));
            }
        }

        Ok(events)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_created() {
        let obs = RawObservation {
            path: PathBuf::from("a.txt"),
            operation: RawOperation::Created,
        };
        let events = Translator::translate(&obs).unwrap();
        assert_eq!(events.len(), 1);

        match &events[0] {
            EventPayload::FileChanged(fc) => {
                assert_eq!(fc.path, PathBuf::from("a.txt"));
                assert_eq!(fc.operation, FileOperation::Created);
            }
            _ => panic!("expected FileChanged variant"),
        }
    }

    #[test]
    fn translate_modified() {
        let obs = RawObservation {
            path: PathBuf::from("b.txt"),
            operation: RawOperation::Modified,
        };
        let events = Translator::translate(&obs).unwrap();
        assert_eq!(events.len(), 1);

        match &events[0] {
            EventPayload::FileChanged(fc) => {
                assert_eq!(fc.operation, FileOperation::Modified);
            }
            _ => panic!("expected FileChanged variant"),
        }
    }

    #[test]
    fn translate_deleted() {
        let obs = RawObservation {
            path: PathBuf::from("c.txt"),
            operation: RawOperation::Deleted,
        };
        let events = Translator::translate(&obs).unwrap();
        assert_eq!(events.len(), 1);

        match &events[0] {
            EventPayload::FileChanged(fc) => {
                assert_eq!(fc.operation, FileOperation::Deleted);
            }
            _ => panic!("expected FileChanged variant"),
        }
    }

    #[test]
    fn translate_renamed_produces_two_events() {
        let obs = RawObservation {
            path: PathBuf::from("new.txt"),
            operation: RawOperation::Renamed {
                from: PathBuf::from("old.txt"),
            },
        };
        let events = Translator::translate(&obs).unwrap();
        assert_eq!(events.len(), 2);

        match &events[0] {
            EventPayload::FileChanged(fc) => {
                assert_eq!(fc.path, PathBuf::from("old.txt"));
                assert_eq!(fc.operation, FileOperation::Deleted);
            }
            _ => panic!("expected FileChanged variant"),
        }

        match &events[1] {
            EventPayload::FileChanged(fc) => {
                assert_eq!(fc.path, PathBuf::from("new.txt"));
                assert_eq!(fc.operation, FileOperation::Created);
            }
            _ => panic!("expected FileChanged variant"),
        }
    }
}
