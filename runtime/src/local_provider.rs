use contract::{CapabilityRequest, CapabilityResponse, ProviderHealth, ProviderInfo, ProviderState};
use crate::lifecycle::ProviderLifecycle;
use crate::adapter::{adapt_request, adapt_response};
use crate::{CapabilityProvider, LocalModel, ProviderError, ResolvedConfig, RuntimeError};
use std::marker::PhantomData;

// ---------------------------------------------------------------------------
// LocalProvider
//
// Wraps a LocalModel and owns lifecycle management. The provider
// translates between CapabilityRequest and InferenceRequest.
// ---------------------------------------------------------------------------

pub struct LocalProvider<M: LocalModel> {
    lifecycle: ProviderLifecycle,
    config: ResolvedConfig,
    model: M,
    info: ProviderInfo,
    _marker: PhantomData<M>,
}

impl<M: LocalModel> LocalProvider<M> {
    pub fn new(model: M, config: ResolvedConfig, info: ProviderInfo) -> Self {
        Self {
            lifecycle: ProviderLifecycle::new(),
            config,
            model,
            info,
            _marker: PhantomData,
        }
    }

    pub fn state(&self) -> ProviderState {
        self.lifecycle.state()
    }

    pub fn load(&mut self) -> Result<(), ProviderError> {
        self.lifecycle.load()?;

        match self.model.load(&self.config) {
            Ok(()) => self.lifecycle.loaded()?,
            Err(err) => {
                self.lifecycle.failed()?;
                return Err(err);
            }
        }

        Ok(())
    }

    pub fn unload(&mut self) -> Result<(), ProviderError> {
        self.lifecycle.unload()?;

        match self.model.unload() {
            Ok(()) => self.lifecycle.unloaded()?,
            Err(err) => {
                self.lifecycle.failed()?;
                return Err(err);
            }
        }

        Ok(())
    }
}

impl<M: LocalModel> CapabilityProvider for LocalProvider<M> {
    fn info(&self) -> ProviderInfo {
        self.info.clone()
    }

    fn health(&self) -> ProviderHealth {
        match self.lifecycle.state() {
            ProviderState::Ready => ProviderHealth::Healthy,
            ProviderState::Loading | ProviderState::Unloading => {
                ProviderHealth::Degraded { reason: "busy" }
            }
            ProviderState::Failed => ProviderHealth::Unhealthy {
                reason: "model failed",
            },
            ProviderState::Uninitialized => ProviderHealth::Degraded {
                reason: "not loaded",
            },
        }
    }

    fn execute(
        &self,
        request: CapabilityRequest,
    ) -> Result<CapabilityResponse, RuntimeError> {
        if self.lifecycle.state() != ProviderState::Ready {
            return Err(RuntimeError::Provider("not ready".into()));
        }

        let runtime_req = adapt_request(request)?;
        let inference_resp = self.model.infer(crate::InferenceRequest {
            prompt: runtime_req.prompt,
            temperature: runtime_req.temperature,
            max_tokens: runtime_req.max_tokens,
        })?;
        adapt_response(inference_resp)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MockLocalModel;

    fn test_info() -> ProviderInfo {
        ProviderInfo {
            name: "test-provider".into(),
            version: "0.1.0".into(),
            supported_capabilities: vec![
                contract::CapabilityKind::BuildSummary,
                contract::CapabilityKind::ExplainRust,
            ],
        }
    }

    #[test]
    fn local_provider_starts_uninitialized() {
        let model = MockLocalModel::new();
        let provider = LocalProvider::new(model, ResolvedConfig::default(), test_info());
        assert_eq!(provider.state(), ProviderState::Uninitialized);
    }

    #[test]
    fn local_provider_load_transitions_to_ready() {
        let model = MockLocalModel::new();
        let mut provider = LocalProvider::new(model, ResolvedConfig::default(), test_info());
        provider.load().unwrap();
        assert_eq!(provider.state(), ProviderState::Ready);
    }

    #[test]
    fn local_provider_unload_transitions_to_uninitialized() {
        let model = MockLocalModel::new();
        let mut provider = LocalProvider::new(model, ResolvedConfig::default(), test_info());
        provider.load().unwrap();
        provider.unload().unwrap();
        assert_eq!(provider.state(), ProviderState::Uninitialized);
    }

    #[test]
    fn local_provider_execute_fails_if_not_ready() {
        let model = MockLocalModel::new();
        let provider = LocalProvider::new(model, ResolvedConfig::default(), test_info());
        let request = CapabilityRequest::BuildSummary(contract::BuildSummaryRequest {
            metadata: contract::RequestMetadata::new(),
            command: "test".into(),
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            prompt: "test prompt".into(),
            temperature: 0.1,
            max_tokens: 100,
        });
        assert!(provider.execute(request).is_err());
    }

    #[test]
    fn local_provider_execute_translates_correctly() {
        let model = MockLocalModel::new();
        let mut provider = LocalProvider::new(model, ResolvedConfig::default(), test_info());
        provider.load().unwrap();

        let request = CapabilityRequest::BuildSummary(contract::BuildSummaryRequest {
            metadata: contract::RequestMetadata::new(),
            command: "test".into(),
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            prompt: "test prompt".into(),
            temperature: 0.1,
            max_tokens: 100,
        });

        let response = provider.execute(request).unwrap();
        match response {
            CapabilityResponse::BuildSummary(resp) => {
                assert!(resp.success);
                assert!(resp.summary.contains("mock: test prompt"));
            }
            _ => panic!("unexpected response variant"),
        }
    }

    #[test]
    fn local_provider_health_reflects_state() {
        let model = MockLocalModel::new();
        let mut provider = LocalProvider::new(model, ResolvedConfig::default(), test_info());

        assert!(matches!(
            provider.health(),
            ProviderHealth::Degraded { reason: "not loaded" }
        ));

        provider.load().unwrap();
        assert!(matches!(provider.health(), ProviderHealth::Healthy));

        provider.unload().unwrap();
        assert!(matches!(
            provider.health(),
            ProviderHealth::Degraded { reason: "not loaded" }
        ));
    }

    #[test]
    fn local_provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LocalProvider<MockLocalModel>>();
    }
}
