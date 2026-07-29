use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Provider state
//
// Observable lifecycle states for an inference provider. Failed is not
// terminal — recovery is another load() attempt.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderState {
    Uninitialized,
    Loading,
    Ready,
    Failed,
    Unloading,
}

// ---------------------------------------------------------------------------
// State errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProviderStateError {
    #[error("cannot load from {current:?}")]
    InvalidLoad { current: ProviderState },

    #[error("cannot unload from {current:?}")]
    InvalidUnload { current: ProviderState },
}

// ---------------------------------------------------------------------------
// Transitions
//
// The caller drives the state machine. Each method consumes self and
// returns the next state on success.
// ---------------------------------------------------------------------------

impl ProviderState {
    /// Transition to Loading.
    ///
    /// Valid from: Uninitialized, Failed.
    pub fn load(self) -> Result<Self, ProviderStateError> {
        match self {
            Self::Uninitialized | Self::Failed => Ok(Self::Loading),
            current => Err(ProviderStateError::InvalidLoad { current }),
        }
    }

    /// Transition to Ready.
    ///
    /// Valid from: Loading.
    pub fn loaded(self) -> Result<Self, ProviderStateError> {
        Ok(Self::Ready)
    }

    /// Transition to Failed.
    ///
    /// Valid from: Loading.
    pub fn failed(self) -> Result<Self, ProviderStateError> {
        Ok(Self::Failed)
    }

    /// Transition to Unloading.
    ///
    /// Valid from: Ready.
    pub fn unload(self) -> Result<Self, ProviderStateError> {
        match self {
            Self::Ready => Ok(Self::Unloading),
            current => Err(ProviderStateError::InvalidUnload { current }),
        }
    }

    /// Transition to Uninitialized.
    ///
    /// Valid from: Unloading.
    pub fn unloaded(self) -> Result<Self, ProviderStateError> {
        Ok(Self::Uninitialized)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uninitialized_loads_to_loading() {
        let state = ProviderState::Uninitialized;
        assert_eq!(state.load().unwrap(), ProviderState::Loading);
    }

    #[test]
    fn failed_loads_to_loading() {
        let state = ProviderState::Failed;
        assert_eq!(state.load().unwrap(), ProviderState::Loading);
    }

    #[test]
    fn loading_loaded_to_ready() {
        let state = ProviderState::Loading;
        assert_eq!(state.loaded().unwrap(), ProviderState::Ready);
    }

    #[test]
    fn loading_failed_to_failed() {
        let state = ProviderState::Loading;
        assert_eq!(state.failed().unwrap(), ProviderState::Failed);
    }

    #[test]
    fn ready_unloads_to_unloading() {
        let state = ProviderState::Ready;
        assert_eq!(state.unload().unwrap(), ProviderState::Unloading);
    }

    #[test]
    fn unloading_unloaded_to_uninitialized() {
        let state = ProviderState::Unloading;
        assert_eq!(state.unloaded().unwrap(), ProviderState::Uninitialized);
    }

    #[test]
    fn invalid_load_from_loading() {
        let state = ProviderState::Loading;
        assert!(state.load().is_err());
    }

    #[test]
    fn invalid_load_from_ready() {
        let state = ProviderState::Ready;
        assert!(state.load().is_err());
    }

    #[test]
    fn invalid_load_from_unloading() {
        let state = ProviderState::Unloading;
        assert!(state.load().is_err());
    }

    #[test]
    fn invalid_unload_from_uninitialized() {
        let state = ProviderState::Uninitialized;
        assert!(state.unload().is_err());
    }

    #[test]
    fn invalid_unload_from_loading() {
        let state = ProviderState::Loading;
        assert!(state.unload().is_err());
    }

    #[test]
    fn invalid_unload_from_failed() {
        let state = ProviderState::Failed;
        assert!(state.unload().is_err());
    }

    #[test]
    fn full_lifecycle_uninitialized_to_ready_to_uninitialized() {
        let state = ProviderState::Uninitialized;
        let state = state.load().unwrap();
        assert_eq!(state, ProviderState::Loading);
        let state = state.loaded().unwrap();
        assert_eq!(state, ProviderState::Ready);
        let state = state.unload().unwrap();
        assert_eq!(state, ProviderState::Unloading);
        let state = state.unloaded().unwrap();
        assert_eq!(state, ProviderState::Uninitialized);
    }

    #[test]
    fn failed_can_retry_to_loading() {
        let state = ProviderState::Loading;
        let state = state.failed().unwrap();
        assert_eq!(state, ProviderState::Failed);
        let state = state.load().unwrap();
        assert_eq!(state, ProviderState::Loading);
        let state = state.loaded().unwrap();
        assert_eq!(state, ProviderState::Ready);
    }

    #[test]
    fn state_is_clone() {
        let state = ProviderState::Ready;
        let _cloned = state;
    }

    #[test]
    fn state_is_debug() {
        let state = ProviderState::Ready;
        let _debug = format!("{:?}", state);
    }

    #[test]
    fn state_error_display() {
        let err = ProviderStateError::InvalidLoad {
            current: ProviderState::Ready,
        };
        assert_eq!(err.to_string(), "cannot load from Ready");

        let err = ProviderStateError::InvalidUnload {
            current: ProviderState::Loading,
        };
        assert_eq!(err.to_string(), "cannot unload from Loading");
    }
}
