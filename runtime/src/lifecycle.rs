use crate::ProviderError;
use contract::ProviderState;

// ---------------------------------------------------------------------------
// ProviderLifecycle
//
// Wraps ProviderState and owns transition recovery. The lifecycle is
// never left in an intermediate state after an error.
// ---------------------------------------------------------------------------

pub struct ProviderLifecycle {
    state: ProviderState,
}

impl ProviderLifecycle {
    pub fn new() -> Self {
        Self {
            state: ProviderState::Uninitialized,
        }
    }

    pub fn state(&self) -> ProviderState {
        self.state
    }

    pub fn load(&mut self) -> Result<(), ProviderError> {
        self.state = self.state.load()?;
        Ok(())
    }

    pub fn loaded(&mut self) -> Result<(), ProviderError> {
        self.state = self.state.loaded()?;
        Ok(())
    }

    pub fn failed(&mut self) -> Result<(), ProviderError> {
        self.state = self.state.failed()?;
        Ok(())
    }

    pub fn unload(&mut self) -> Result<(), ProviderError> {
        self.state = self.state.unload()?;
        Ok(())
    }

    pub fn unloaded(&mut self) -> Result<(), ProviderError> {
        self.state = self.state.unloaded()?;
        Ok(())
    }
}

impl Default for ProviderLifecycle {
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

    #[test]
    fn lifecycle_starts_uninitialized() {
        let lifecycle = ProviderLifecycle::new();
        assert_eq!(lifecycle.state(), ProviderState::Uninitialized);
    }

    #[test]
    fn lifecycle_load_transitions_to_loading() {
        let mut lifecycle = ProviderLifecycle::new();
        lifecycle.load().unwrap();
        assert_eq!(lifecycle.state(), ProviderState::Loading);
    }

    #[test]
    fn lifecycle_loaded_transitions_to_ready() {
        let mut lifecycle = ProviderLifecycle::new();
        lifecycle.load().unwrap();
        lifecycle.loaded().unwrap();
        assert_eq!(lifecycle.state(), ProviderState::Ready);
    }

    #[test]
    fn lifecycle_failed_transitions_to_failed() {
        let mut lifecycle = ProviderLifecycle::new();
        lifecycle.load().unwrap();
        lifecycle.failed().unwrap();
        assert_eq!(lifecycle.state(), ProviderState::Failed);
    }

    #[test]
    fn lifecycle_unload_transitions_to_unloading() {
        let mut lifecycle = ProviderLifecycle::new();
        lifecycle.load().unwrap();
        lifecycle.loaded().unwrap();
        lifecycle.unload().unwrap();
        assert_eq!(lifecycle.state(), ProviderState::Unloading);
    }

    #[test]
    fn lifecycle_unloaded_transitions_to_uninitialized() {
        let mut lifecycle = ProviderLifecycle::new();
        lifecycle.load().unwrap();
        lifecycle.loaded().unwrap();
        lifecycle.unload().unwrap();
        lifecycle.unloaded().unwrap();
        assert_eq!(lifecycle.state(), ProviderState::Uninitialized);
    }

    #[test]
    fn lifecycle_default() {
        let lifecycle = ProviderLifecycle::default();
        assert_eq!(lifecycle.state(), ProviderState::Uninitialized);
    }
}
