use std::sync::{Arc, Mutex};

use crate::factory::ProviderFactory;
use crate::{CapabilityProvider, RuntimeError};
use contract::{CapabilityRequest, CapabilityResponse, ProviderHealth, ProviderInfo};

enum State {
    Uninitialized,
    Loading,
    Ready(Arc<dyn CapabilityProvider + Send + Sync>),
    Failed(String),
}

pub struct LazyProvider<F: ProviderFactory> {
    factory: Mutex<Option<F>>,
    state: Mutex<State>,
}

impl<F: ProviderFactory> LazyProvider<F> {
    pub fn new(factory: F) -> Self {
        Self {
            factory: Mutex::new(Some(factory)),
            state: Mutex::new(State::Uninitialized),
        }
    }

    fn ensure_ready(&self) -> Result<(), RuntimeError> {
        let mut state = self.state.lock().unwrap();
        match &*state {
            State::Ready(_) => return Ok(()),
            State::Failed(msg) => return Err(RuntimeError::Provider(msg.clone().into())),
            State::Loading => return Err(RuntimeError::Provider("model is loading".into())),
            State::Uninitialized => {}
        }

        let factory = self
            .factory
            .lock()
            .unwrap()
            .take()
            .ok_or_else(|| RuntimeError::Provider("factory consumed".into()))?;

        *state = State::Loading;
        drop(state);

        match factory.create() {
            Ok(provider) => {
                let mut state = self.state.lock().unwrap();
                *state = State::Ready(provider);
                Ok(())
            }
            Err(e) => {
                let mut state = self.state.lock().unwrap();
                *state = State::Failed(e.to_string());
                Err(e)
            }
        }
    }
}

impl<F: ProviderFactory> CapabilityProvider for LazyProvider<F> {
    fn info(&self) -> ProviderInfo {
        let state = self.state.lock().unwrap();
        match &*state {
            State::Ready(provider) => provider.info(),
            _ => ProviderInfo {
                name: "lazy".into(),
                version: "uninitialized".into(),
                supported_capabilities: vec![],
            },
        }
    }

    fn health(&self) -> ProviderHealth {
        let state = self.state.lock().unwrap();
        match &*state {
            State::Ready(provider) => provider.health(),
            State::Failed(_) => ProviderHealth::Unhealthy {
                reason: "model failed to load",
            },
            _ => ProviderHealth::Unhealthy {
                reason: "model not loaded",
            },
        }
    }

    fn execute(&self, request: CapabilityRequest) -> Result<CapabilityResponse, RuntimeError> {
        self.ensure_ready()?;
        let state = self.state.lock().unwrap();
        match &*state {
            State::Ready(provider) => provider.execute(request),
            _ => Err(RuntimeError::Provider("model not ready".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contract::CapabilityKind;

    struct DummyFactory;

    impl ProviderFactory for DummyFactory {
        fn create(&self) -> Result<Arc<dyn CapabilityProvider + Send + Sync>, RuntimeError> {
            struct DummyProvider;
            impl CapabilityProvider for DummyProvider {
                fn info(&self) -> ProviderInfo {
                    ProviderInfo {
                        name: "dummy".into(),
                        version: "0.1.0".into(),
                        supported_capabilities: vec![CapabilityKind::BuildSummary],
                    }
                }
                fn health(&self) -> ProviderHealth {
                    ProviderHealth::Healthy
                }
                fn execute(
                    &self,
                    request: CapabilityRequest,
                ) -> Result<CapabilityResponse, RuntimeError> {
                    match request {
                        CapabilityRequest::BuildSummary(_) => Ok(CapabilityResponse::BuildSummary(
                            contract::BuildSummaryResponse {
                                success: true,
                                summary: "dummy response".into(),
                                recommendation: None,
                            },
                        )),
                        _ => Err(RuntimeError::InvalidResponse),
                    }
                }
            }
            Ok(Arc::new(DummyProvider))
        }
    }

    struct FailingFactory;

    impl ProviderFactory for FailingFactory {
        fn create(&self) -> Result<Arc<dyn CapabilityProvider + Send + Sync>, RuntimeError> {
            Err(RuntimeError::Provider("intentional failure".into()))
        }
    }

    #[test]
    fn lazy_provider_defers_creation() {
        let provider = LazyProvider::new(DummyFactory);
        assert_eq!(provider.info().name, "lazy");

        let request = CapabilityRequest::BuildSummary(contract::BuildSummaryRequest {
            metadata: contract::RequestMetadata::new(),
            command: "test".into(),
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            prompt: "test".into(),
            temperature: 0.1,
            max_tokens: 64,
        });

        let _response = provider.execute(request).unwrap();
        assert_eq!(provider.info().name, "dummy");
    }

    #[test]
    fn lazy_provider_caches_after_success() {
        let provider = LazyProvider::new(DummyFactory);

        let request = CapabilityRequest::BuildSummary(contract::BuildSummaryRequest {
            metadata: contract::RequestMetadata::new(),
            command: "test".into(),
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            prompt: "test".into(),
            temperature: 0.1,
            max_tokens: 64,
        });

        provider.execute(request.clone()).unwrap();
        provider.execute(request).unwrap();
        assert_eq!(provider.info().name, "dummy");
    }

    #[test]
    fn lazy_provider_fails_permanently() {
        let provider = LazyProvider::new(FailingFactory);

        let request = CapabilityRequest::BuildSummary(contract::BuildSummaryRequest {
            metadata: contract::RequestMetadata::new(),
            command: "test".into(),
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            prompt: "test".into(),
            temperature: 0.1,
            max_tokens: 64,
        });

        let result = provider.execute(request.clone());
        assert!(result.is_err());

        let result = provider.execute(request);
        assert!(result.is_err());
        assert_eq!(provider.info().name, "lazy");
    }
}
