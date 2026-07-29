use uuid::Uuid;

// ---------------------------------------------------------------------------
// Observation (Projected)
// ---------------------------------------------------------------------------

/// A projected observation consumed by the agent layer.
///
/// The agent layer MUST NOT import raw kernel events. Observations
/// are projected state provided by the projection layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    /// Unique identifier for this observation.
    pub id: Uuid,
    /// Human-readable description of what was observed.
    pub description: String,
    /// Timestamp when the observation was made.
    pub timestamp: std::time::SystemTime,
}

// ---------------------------------------------------------------------------
// Agent Context
// ---------------------------------------------------------------------------

/// Context available to the agent layer during reasoning.
///
/// Contains projected observations and execution history. The agent layer
/// MUST NOT consume raw kernel events — it uses projected types only.
///
/// ```text
/// Kernel Event → Projection Layer → AgentContext
/// ```
#[derive(Debug, Clone, Default)]
pub struct AgentContext {
    /// Recent observations from the projection layer.
    pub observations: Vec<Observation>,
    /// History of past agent results.
    pub history: Vec<crate::result::AgentResult>,
}

impl AgentContext {
    /// Create an empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an observation to the context.
    pub fn add_observation(&mut self, observation: Observation) {
        self.observations.push(observation);
    }

    /// Add a result to the history.
    pub fn add_result(&mut self, result: crate::result::AgentResult) {
        self.history.push(result);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_starts_empty() {
        let ctx = AgentContext::new();
        assert!(ctx.observations.is_empty());
        assert!(ctx.history.is_empty());
    }

    #[test]
    fn add_observation() {
        let mut ctx = AgentContext::new();
        let obs = Observation {
            id: Uuid::new_v4(),
            description: "file changed".into(),
            timestamp: std::time::SystemTime::now(),
        };
        ctx.add_observation(obs.clone());
        assert_eq!(ctx.observations.len(), 1);
        assert_eq!(ctx.observations[0], obs);
    }
}
