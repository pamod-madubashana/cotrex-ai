use std::sync::Arc;

use crate::{CapabilityProvider, RuntimeError};

pub trait ProviderFactory: Send + Sync + 'static {
    fn create(&self) -> Result<Arc<dyn CapabilityProvider + Send + Sync>, RuntimeError>;
}
