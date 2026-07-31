use crate::adapter::{adapt_request, adapt_response};
use crate::lifecycle::ProviderLifecycle;
use crate::{CapabilityProvider, LocalModel, ProviderError, ResolvedConfig, RuntimeError};
use contract::{
    CapabilityRequest, CapabilityResponse, ProviderHealth, ProviderInfo, ProviderState,
};
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

    fn execute(&self, request: CapabilityRequest) -> Result<CapabilityResponse, RuntimeError> {
        if self.lifecycle.state() != ProviderState::Ready {
            return Err(RuntimeError::Provider("not ready".into()));
        }

        let runtime_req = adapt_request(request)?;
        let inference_resp = self.model.infer(crate::InferenceRequest {
            prompt: runtime_req.prompt,
            messages: vec![],
            temperature: runtime_req.temperature,
            max_tokens: runtime_req.max_tokens,
            token_callback: None,
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
            ProviderHealth::Degraded {
                reason: "not loaded"
            }
        ));

        provider.load().unwrap();
        assert!(matches!(provider.health(), ProviderHealth::Healthy));

        provider.unload().unwrap();
        assert!(matches!(
            provider.health(),
            ProviderHealth::Degraded {
                reason: "not loaded"
            }
        ));
    }

    #[test]
    fn local_provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LocalProvider<MockLocalModel>>();
    }

    // =========================================================================
    // Validation tests for RFC-0007 invariants
    // =========================================================================

    #[test]
    fn provider_starts_uninitialized() {
        let model = MockLocalModel::new();
        let provider = LocalProvider::new(model, ResolvedConfig::default(), test_info());
        assert_eq!(provider.state(), ProviderState::Uninitialized);
    }

    #[test]
    fn load_transitions_to_ready() {
        let model = MockLocalModel::new();
        let mut provider = LocalProvider::new(model, ResolvedConfig::default(), test_info());
        provider.load().unwrap();
        assert_eq!(provider.state(), ProviderState::Ready);
    }

    #[test]
    fn failed_load_transitions_to_failed() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        struct FailingModel {
            call_count: Arc<AtomicUsize>,
        }

        impl crate::LocalModel for FailingModel {
            fn load(
                &mut self,
                _config: &crate::ResolvedConfig,
            ) -> Result<(), crate::ProviderError> {
                self.call_count.fetch_add(1, Ordering::SeqCst);
                Err(crate::ProviderError::Model("load failed".into()))
            }
            fn infer(
                &self,
                _req: crate::InferenceRequest,
            ) -> Result<crate::InferenceResponse, crate::ProviderError> {
                unreachable!("should not be called")
            }
            fn unload(&mut self) -> Result<(), crate::ProviderError> {
                Ok(())
            }
            fn info(&self) -> crate::ModelInfo {
                crate::ModelInfo {
                    name: "failing".into(),
                    version: "0.0.0".into(),
                    backend: "test".into(),
                }
            }
        }

        let call_count = Arc::new(AtomicUsize::new(0));
        let model = FailingModel {
            call_count: call_count.clone(),
        };
        let mut provider = LocalProvider::new(model, ResolvedConfig::default(), test_info());

        let result = provider.load();
        assert!(result.is_err());
        assert_eq!(provider.state(), ProviderState::Failed);
    }

    #[test]
    fn failed_can_retry_to_loading() {
        use crate::ProviderError;

        struct FailOnceModel {
            call_count: Arc<AtomicUsize>,
        }

        impl crate::LocalModel for FailOnceModel {
            fn load(
                &mut self,
                _config: &crate::ResolvedConfig,
            ) -> Result<(), crate::ProviderError> {
                let count = self.call_count.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    Err(ProviderError::Model("first call fails".into()))
                } else {
                    Ok(())
                }
            }
            fn infer(
                &self,
                _req: crate::InferenceRequest,
            ) -> Result<crate::InferenceResponse, crate::ProviderError> {
                Ok(crate::InferenceResponse { text: "ok".into(), profile: None })
            }
            fn unload(&mut self) -> Result<(), crate::ProviderError> {
                Ok(())
            }
            fn info(&self) -> crate::ModelInfo {
                crate::ModelInfo {
                    name: "fail-once".into(),
                    version: "0.0.0".into(),
                    backend: "test".into(),
                }
            }
        }

        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        let call_count = Arc::new(AtomicUsize::new(0));
        let model = FailOnceModel {
            call_count: call_count.clone(),
        };
        let mut provider = LocalProvider::new(model, ResolvedConfig::default(), test_info());

        // First load fails
        assert!(provider.load().is_err());
        assert_eq!(provider.state(), ProviderState::Failed);

        // Retry succeeds
        assert!(provider.load().is_ok());
        assert_eq!(provider.state(), ProviderState::Ready);
    }

    #[test]
    fn unload_transitions_to_uninitialized() {
        let model = MockLocalModel::new();
        let mut provider = LocalProvider::new(model, ResolvedConfig::default(), test_info());
        provider.load().unwrap();
        provider.unload().unwrap();
        assert_eq!(provider.state(), ProviderState::Uninitialized);
    }

    #[test]
    fn provider_owns_exactly_one_model() {
        struct CountingModel {
            instance_id: usize,
        }

        impl crate::LocalModel for CountingModel {
            fn load(
                &mut self,
                _config: &crate::ResolvedConfig,
            ) -> Result<(), crate::ProviderError> {
                Ok(())
            }
            fn infer(
                &self,
                _req: crate::InferenceRequest,
            ) -> Result<crate::InferenceResponse, crate::ProviderError> {
                Ok(crate::InferenceResponse {
                    text: format!("model-{}", self.instance_id),
                    profile: None,
                })
            }
            fn unload(&mut self) -> Result<(), crate::ProviderError> {
                Ok(())
            }
            fn info(&self) -> crate::ModelInfo {
                crate::ModelInfo {
                    name: "counting".into(),
                    version: "0.0.0".into(),
                    backend: "test".into(),
                }
            }
        }

        let model = CountingModel { instance_id: 42 };
        let mut provider = LocalProvider::new(model, ResolvedConfig::default(), test_info());
        provider.load().unwrap();

        // Multiple inferences should use the same model instance
        let req = crate::InferenceRequest {
            prompt: crate::Prompt::new("test"),
            messages: vec![],
            temperature: 0.1,
            max_tokens: 100,
            token_callback: None,
        };

        let resp1 = provider.model.infer(req.clone()).unwrap();
        let resp2 = provider.model.infer(req).unwrap();

        assert_eq!(resp1.text, "model-42");
        assert_eq!(resp2.text, "model-42");
    }

    #[test]
    fn multiple_infer_calls_reuse_same_model() {
        let model = MockLocalModel::new();
        let mut provider = LocalProvider::new(model, ResolvedConfig::default(), test_info());
        provider.load().unwrap();

        let req = crate::InferenceRequest {
            prompt: crate::Prompt::new("test"),
            messages: vec![],
            temperature: 0.1,
            max_tokens: 100,
            token_callback: None,
        };

        // Both calls should succeed using the same model
        let resp1 = provider.model.infer(req.clone()).unwrap();
        let resp2 = provider.model.infer(req).unwrap();

        assert_eq!(resp1.text, resp2.text);
    }

    #[test]
    fn no_inference_while_not_ready() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        struct TrackInferenceModel {
            infer_count: Arc<AtomicUsize>,
        }

        impl crate::LocalModel for TrackInferenceModel {
            fn load(
                &mut self,
                _config: &crate::ResolvedConfig,
            ) -> Result<(), crate::ProviderError> {
                Ok(())
            }
            fn infer(
                &self,
                _req: crate::InferenceRequest,
            ) -> Result<crate::InferenceResponse, crate::ProviderError> {
                self.infer_count.fetch_add(1, Ordering::SeqCst);
                Ok(crate::InferenceResponse { text: "ok".into(), profile: None })
            }
            fn unload(&mut self) -> Result<(), crate::ProviderError> {
                Ok(())
            }
            fn info(&self) -> crate::ModelInfo {
                crate::ModelInfo {
                    name: "tracking".into(),
                    version: "0.0.0".into(),
                    backend: "test".into(),
                }
            }
        }

        let infer_count = Arc::new(AtomicUsize::new(0));
        let model = TrackInferenceModel {
            infer_count: infer_count.clone(),
        };
        let provider = LocalProvider::new(model, ResolvedConfig::default(), test_info());

        // Provider is Uninitialized — execute should fail without calling infer
        let request = CapabilityRequest::BuildSummary(contract::BuildSummaryRequest {
            metadata: contract::RequestMetadata::new(),
            command: "test".into(),
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            prompt: "test".into(),
            temperature: 0.1,
            max_tokens: 100,
        });

        let result = provider.execute(request);
        assert!(result.is_err());
        assert_eq!(infer_count.load(Ordering::SeqCst), 0); // No inference happened
    }
}
