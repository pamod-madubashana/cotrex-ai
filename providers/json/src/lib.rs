use contract::{
    BuildSummaryRequest, BuildSummaryResponse, CapabilityKind, CapabilityRequest,
    CapabilityResponse, ExplainRustResponse, ProviderHealth, ProviderInfo,
};
use runtime::{CapabilityProvider, RuntimeError};
use std::path::PathBuf;

/// JSON fixture provider. Loads responses from JSON files on disk.
/// Proves protocol + serialization work end-to-end without AI.
pub struct JsonProvider {
    fixtures_dir: PathBuf,
}

impl JsonProvider {
    pub fn new(fixtures_dir: impl Into<PathBuf>) -> Self {
        Self {
            fixtures_dir: fixtures_dir.into(),
        }
    }

    fn load_fixture<T: serde::de::DeserializeOwned>(&self, name: &str) -> Result<T, RuntimeError> {
        let path = self.fixtures_dir.join(name);
        let data = std::fs::read_to_string(&path).map_err(|e| {
            RuntimeError::Provider(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("failed to read fixture {}: {}", path.display(), e),
            )))
        })?;
        serde_json::from_str(&data).map_err(|e| {
            RuntimeError::Provider(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("failed to parse fixture {}: {}", path.display(), e),
            )))
        })
    }
}

impl CapabilityProvider for JsonProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "json-fixture".into(),
            version: "0.1.0".into(),
            supported_capabilities: vec![CapabilityKind::BuildSummary, CapabilityKind::ExplainRust],
        }
    }

    fn health(&self) -> ProviderHealth {
        ProviderHealth::Healthy
    }

    fn execute(&self, request: CapabilityRequest) -> Result<CapabilityResponse, RuntimeError> {
        match request {
            CapabilityRequest::BuildSummary(req) => {
                let resp: BuildSummaryResponse = self.determine_build_fixture(&req)?;
                Ok(CapabilityResponse::BuildSummary(resp))
            }
            CapabilityRequest::ExplainRust(_req) => {
                let resp: ExplainRustResponse = self.load_fixture("rust_explain.json")?;
                Ok(CapabilityResponse::ExplainRust(resp))
            }
        }
    }
}

impl JsonProvider {
    fn determine_build_fixture(
        &self,
        req: &BuildSummaryRequest,
    ) -> Result<BuildSummaryResponse, RuntimeError> {
        if req.exit_code == 0 {
            self.load_fixture("build_success.json")
        } else {
            self.load_fixture("build_failure.json")
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use contract::RequestMetadata;
    use runtime::CapabilityProviderExt;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
    }

    fn metadata() -> RequestMetadata {
        RequestMetadata::new()
    }

    #[test]
    fn build_success_loads_fixture() {
        let provider = JsonProvider::new(fixtures_dir());
        let resp = provider
            .build_summary(BuildSummaryRequest {
                metadata: metadata(),
                command: "cargo build".into(),
                exit_code: 0,
                stdout: "Finished dev profile".into(),
                stderr: String::new(),
            })
            .unwrap();
        assert!(resp.success);
        assert_eq!(resp.summary, "Build completed successfully.");
        assert!(resp.recommendation.is_none());
    }

    #[test]
    fn build_failure_loads_fixture() {
        let provider = JsonProvider::new(fixtures_dir());
        let resp = provider
            .build_summary(BuildSummaryRequest {
                metadata: metadata(),
                command: "cargo build".into(),
                exit_code: 101,
                stdout: String::new(),
                stderr: "error[E0308]: mismatched types".into(),
            })
            .unwrap();
        assert!(!resp.success);
        assert_eq!(resp.summary, "Compilation failed.");
        assert_eq!(
            resp.recommendation.unwrap(),
            "Inspect compiler diagnostics."
        );
    }

    #[test]
    fn explain_rust_loads_fixture() {
        let provider = JsonProvider::new(fixtures_dir());
        let resp = provider
            .explain_rust(ExplainRustRequest {
                metadata: metadata(),
                source: "fn fibonacci(n: u32) -> u32 { n }".into(),
                question: "what does this do?".into(),
            })
            .unwrap();
        assert!(resp.explanation.contains("Fibonacci"));
    }

    #[test]
    fn execute_returns_matching_variant() {
        let provider = JsonProvider::new(fixtures_dir());
        let resp = provider
            .execute(CapabilityRequest::BuildSummary(BuildSummaryRequest {
                metadata: metadata(),
                command: "cargo test".into(),
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
            }))
            .unwrap();
        assert!(matches!(resp, CapabilityResponse::BuildSummary(_)));
    }

    #[test]
    fn missing_fixture_returns_error() {
        let provider = JsonProvider::new("/nonexistent/path");
        let resp = provider.build_summary(BuildSummaryRequest {
            metadata: metadata(),
            command: "cargo build".into(),
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        });
        assert!(resp.is_err());
    }
}
