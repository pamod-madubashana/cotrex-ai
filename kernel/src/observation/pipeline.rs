use crate::observation::filter::{FilterDecision, ObservationFilter};
use crate::observation::translator::{RawObservation, Translator};
use crate::store::{EventStore, EventStoreError};
use std::path::PathBuf;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Observation Status (RFC-0003, Section 11)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationStatus {
    Created,
    Initializing,
    Watching,
    Failed,
    Stopped,
}

impl ObservationStatus {
    pub fn can_transition_to(self, next: ObservationStatus) -> bool {
        matches!(
            (self, next),
            (ObservationStatus::Created, ObservationStatus::Initializing)
                | (ObservationStatus::Initializing, ObservationStatus::Watching)
                | (ObservationStatus::Watching, ObservationStatus::Failed)
                | (ObservationStatus::Failed, ObservationStatus::Initializing)
                | (ObservationStatus::Watching, ObservationStatus::Stopped)
        )
    }
}

// ---------------------------------------------------------------------------
// Pipeline Statistics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct PipelineStats {
    pub accepted: u64,
    pub rejected: u64,
    pub events_created: u64,
}

// ---------------------------------------------------------------------------
// Observation Pipeline (RFC-0003)
// ---------------------------------------------------------------------------

pub struct ObservationPipeline {
    status: Mutex<ObservationStatus>,
    filter: ObservationFilter,
    stats: Mutex<PipelineStats>,
}

impl ObservationPipeline {
    pub fn new(root: PathBuf) -> Self {
        Self {
            status: Mutex::new(ObservationStatus::Created),
            filter: ObservationFilter::new(root),
            stats: Mutex::new(PipelineStats::default()),
        }
    }

    /// Return current pipeline status.
    pub fn status(&self) -> ObservationStatus {
        self.status
            .lock()
            .map(|s| *s)
            .unwrap_or(ObservationStatus::Failed)
    }

    /// Return pipeline statistics.
    pub fn stats(&self) -> PipelineStats {
        self.stats.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Transition to a new status. Returns error on invalid transition.
    fn set_status(&self, next: ObservationStatus) -> Result<(), EventStoreError> {
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

    /// Initialize the pipeline (Created -> Initializing).
    pub fn initialize(&self) -> Result<(), EventStoreError> {
        self.set_status(ObservationStatus::Initializing)
    }

    /// Start watching (Initializing -> Watching).
    pub fn start_watching(&self) -> Result<(), EventStoreError> {
        self.set_status(ObservationStatus::Watching)
    }

    /// Mark as failed (Watching -> Failed).
    pub fn mark_failed(&self) -> Result<(), EventStoreError> {
        self.set_status(ObservationStatus::Failed)
    }

    /// Attempt recovery (Failed -> Initializing).
    pub fn recover(&self) -> Result<(), EventStoreError> {
        self.set_status(ObservationStatus::Initializing)
    }

    /// Stop the pipeline (Watching -> Stopped).
    pub fn stop(&self) -> Result<(), EventStoreError> {
        self.set_status(ObservationStatus::Stopped)
    }

    /// Process a raw observation through filter and translator, then append to store.
    ///
    /// Returns the number of events created (0 if rejected, 1 for most, 2 for renames).
    pub fn process_observation(
        &self,
        observation: &RawObservation,
        store: &EventStore,
    ) -> Result<u64, EventStoreError> {
        // Check status
        if self.status() != ObservationStatus::Watching {
            return Err(EventStoreError::ProjectionFailure(
                "pipeline not watching".to_string(),
            ));
        }

        // Filter
        match self.filter.filter(observation) {
            FilterDecision::Accept => {}
            FilterDecision::Reject { .. } => {
                let mut stats = self.stats.lock().map_err(|e| {
                    EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
                })?;
                stats.rejected += 1;
                return Ok(0);
            }
        }

        // Translate
        let payloads = Translator::translate(observation).map_err(|e| {
            EventStoreError::ProjectionFailure(format!("translation failed: {}", e.reason))
        })?;

        let event_count = payloads.len() as u64;

        // Append to store
        for payload in payloads {
            store.append(payload)?;
        }

        // Update stats
        let mut stats = self.stats.lock().map_err(|e| {
            EventStoreError::ProjectionFailure(format!("failed to acquire lock: {}", e))
        })?;
        stats.accepted += 1;
        stats.events_created += event_count;

        Ok(event_count)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation::translator::RawOperation;
    use crate::store::EventStore;

    fn test_pipeline() -> ObservationPipeline {
        ObservationPipeline::new(PathBuf::from("/project"))
    }

    fn accept_obs(path: &str) -> RawObservation {
        RawObservation {
            path: PathBuf::from(path),
            operation: RawOperation::Created,
        }
    }

    #[test]
    fn pipeline_starts_created() {
        let pipeline = test_pipeline();
        assert_eq!(pipeline.status(), ObservationStatus::Created);
    }

    #[test]
    fn lifecycle_created_to_initializing() {
        let pipeline = test_pipeline();
        pipeline.initialize().unwrap();
        assert_eq!(pipeline.status(), ObservationStatus::Initializing);
    }

    #[test]
    fn lifecycle_initializing_to_watching() {
        let pipeline = test_pipeline();
        pipeline.initialize().unwrap();
        pipeline.start_watching().unwrap();
        assert_eq!(pipeline.status(), ObservationStatus::Watching);
    }

    #[test]
    fn lifecycle_watching_to_failed() {
        let pipeline = test_pipeline();
        pipeline.initialize().unwrap();
        pipeline.start_watching().unwrap();
        pipeline.mark_failed().unwrap();
        assert_eq!(pipeline.status(), ObservationStatus::Failed);
    }

    #[test]
    fn lifecycle_failed_to_initializing() {
        let pipeline = test_pipeline();
        pipeline.initialize().unwrap();
        pipeline.start_watching().unwrap();
        pipeline.mark_failed().unwrap();
        pipeline.recover().unwrap();
        assert_eq!(pipeline.status(), ObservationStatus::Initializing);
    }

    #[test]
    fn lifecycle_watching_to_stopped() {
        let pipeline = test_pipeline();
        pipeline.initialize().unwrap();
        pipeline.start_watching().unwrap();
        pipeline.stop().unwrap();
        assert_eq!(pipeline.status(), ObservationStatus::Stopped);
    }

    #[test]
    fn lifecycle_invalid_created_to_watching() {
        let pipeline = test_pipeline();
        let result = pipeline.start_watching();
        assert!(result.is_err());
    }

    #[test]
    fn lifecycle_invalid_stopped_to_watching() {
        let pipeline = test_pipeline();
        pipeline.initialize().unwrap();
        pipeline.start_watching().unwrap();
        pipeline.stop().unwrap();
        let result = pipeline.start_watching();
        assert!(result.is_err());
    }

    #[test]
    fn process_observation_accepted() {
        let pipeline = test_pipeline();
        let store = EventStore::new();

        pipeline.initialize().unwrap();
        pipeline.start_watching().unwrap();

        let obs = accept_obs("/project/src/main.rs");
        let count = pipeline.process_observation(&obs, &store).unwrap();

        assert_eq!(count, 1);
        assert_eq!(store.len(), 1);

        let stats = pipeline.stats();
        assert_eq!(stats.accepted, 1);
        assert_eq!(stats.rejected, 0);
        assert_eq!(stats.events_created, 1);
    }

    #[test]
    fn process_observation_rejected() {
        let pipeline = test_pipeline();
        let store = EventStore::new();

        pipeline.initialize().unwrap();
        pipeline.start_watching().unwrap();

        let obs = accept_obs("/project/.git/config");
        let count = pipeline.process_observation(&obs, &store).unwrap();

        assert_eq!(count, 0);
        assert_eq!(store.len(), 0);

        let stats = pipeline.stats();
        assert_eq!(stats.accepted, 0);
        assert_eq!(stats.rejected, 1);
        assert_eq!(stats.events_created, 0);
    }

    #[test]
    fn process_observation_not_watching() {
        let pipeline = test_pipeline();
        let store = EventStore::new();

        let obs = accept_obs("/project/src/main.rs");
        let result = pipeline.process_observation(&obs, &store);
        assert!(result.is_err());
    }

    #[test]
    fn process_observation_renamed_produces_two_events() {
        let pipeline = test_pipeline();
        let store = EventStore::new();

        pipeline.initialize().unwrap();
        pipeline.start_watching().unwrap();

        let obs = RawObservation {
            path: PathBuf::from("/project/new.txt"),
            operation: RawOperation::Renamed {
                from: PathBuf::from("/project/old.txt"),
            },
        };
        let count = pipeline.process_observation(&obs, &store).unwrap();

        assert_eq!(count, 2);
        assert_eq!(store.len(), 2);
    }
}
