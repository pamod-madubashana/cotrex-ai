pub mod ai_context;
pub mod engine;
pub mod event;
pub mod observation;
pub mod persistent_store;
pub mod projection;
pub mod store;

pub use ai_context::{AiContextProjection, AiContextSummary, WorkspaceStatus};
pub use engine::{Projection, ProjectionEngine};
pub use event::{
    Event, EventPayload, ExecutionCompleted, ExecutionFailed, ExecutionRequested, FileChanged,
    FileOperation,
};
pub use observation::filter::{FilterDecision, ObservationFilter};
pub use observation::pipeline::{ObservationPipeline, ObservationStatus};
pub use observation::translator::{RawObservation, RawOperation, Translator};
pub use persistent_store::PersistentEventStore;
pub use projection::{FileChangeProjection, ProjectionStatus};
pub use store::{EventStore, EventStoreError, MemoryEventStore, ReplayResult};
