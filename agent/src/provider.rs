use std::collections::HashMap;

use crate::context::AgentContext;
use crate::decision::{AgentDecision, AgentGoal};

// ---------------------------------------------------------------------------
// AI Provider
// ---------------------------------------------------------------------------

/// Future LLM reasoning boundary.
///
/// The provider returns decisions only. It never accesses the execution
/// layer, filesystem, or event store.
pub trait AiProvider {
    /// Produce a decision for the given goal and context.
    fn reason(
        &self,
        goal: &AgentGoal,
        context: &AgentContext,
    ) -> Result<AgentDecision, ProviderError>;
}

/// Errors from the AI provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    /// The model is not available.
    ModelUnavailable(String),
    /// Reasoning failed.
    ReasoningFailed(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ModelUnavailable(msg) => write!(f, "model unavailable: {}", msg),
            Self::ReasoningFailed(msg) => write!(f, "reasoning failed: {}", msg),
        }
    }
}

// ---------------------------------------------------------------------------
// Mock Provider
// ---------------------------------------------------------------------------

/// Deterministic provider for testing.
///
/// Simulates LLM boundary - returns pre-programmed decisions.
/// Does NOT implement planning logic. The mock is a response map,
/// not a hidden rule engine.
pub struct MockProvider {
    responses: HashMap<String, AgentDecision>,
}

impl MockProvider {
    /// Create a new empty mock provider.
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
        }
    }

    /// Add a pre-programmed response for a goal description.
    pub fn with_response(mut self, goal: impl Into<String>, decision: AgentDecision) -> Self {
        self.responses.insert(goal.into(), decision);
        self
    }
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AiProvider for MockProvider {
    fn reason(
        &self,
        goal: &AgentGoal,
        _context: &AgentContext,
    ) -> Result<AgentDecision, ProviderError> {
        Ok(self.responses.get(&goal.description).cloned().unwrap_or(
            AgentDecision::NeedMoreContext(format!("no mock response for: {}", goal.description)),
        ))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_provider_deterministic() {
        let provider =
            MockProvider::new().with_response("test", AgentDecision::Complete("done".into()));
        let goal = AgentGoal::new("test");
        let ctx = AgentContext::new();

        let d1 = provider.reason(&goal, &ctx).unwrap();
        let d2 = provider.reason(&goal, &ctx).unwrap();
        assert_eq!(d1, d2);
    }

    #[test]
    fn mock_provider_known_goal() {
        let provider = MockProvider::new().with_response(
            "create hello.txt",
            AgentDecision::Execute(crate::decision::PlanStep::WriteFile {
                path: "hello.txt".into(),
                content: b"hello".to_vec(),
            }),
        );
        let goal = AgentGoal::new("create hello.txt");
        let ctx = AgentContext::new();

        let decision = provider.reason(&goal, &ctx).unwrap();
        match decision {
            AgentDecision::Execute(crate::decision::PlanStep::WriteFile { path, .. }) => {
                assert_eq!(path, "hello.txt");
            }
            other => panic!("expected WriteFile, got: {:?}", other),
        }
    }

    #[test]
    fn mock_provider_unknown_goal() {
        let provider = MockProvider::new();
        let goal = AgentGoal::new("something random");
        let ctx = AgentContext::new();

        let decision = provider.reason(&goal, &ctx).unwrap();
        match decision {
            AgentDecision::NeedMoreContext(msg) => {
                assert!(msg.contains("something random"));
            }
            other => panic!("expected NeedMoreContext, got: {:?}", other),
        }
    }
}
