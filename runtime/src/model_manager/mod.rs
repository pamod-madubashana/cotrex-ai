pub mod downloader;
pub mod error;
pub mod registry;
pub mod resolver;
pub mod storage;

pub use downloader::download_model;
pub use error::ModelManagerError;
pub use registry::{ModelDefinition, ModelRegistry};
pub use resolver::{ModelResolver, load_registry};

/// Model lifecycle manager. Handles registry, storage, resolution, and download.
pub struct ModelManager;
