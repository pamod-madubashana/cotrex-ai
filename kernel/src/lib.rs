pub mod ai_context;
pub mod engine;
pub mod event;
pub mod projection;
pub mod store;

pub use ai_context::{AiContextProjection, AiContextSummary, WorkspaceStatus};
pub use engine::{Projection, ProjectionEngine};
pub use event::{Event, EventPayload, FileChanged, FileOperation};
pub use projection::{FileChangeProjection, ProjectionStatus};
pub use store::{EventStore, EventStoreError, ReplayResult};
