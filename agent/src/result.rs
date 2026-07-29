use uuid::Uuid;

// ---------------------------------------------------------------------------
// Agent Result
// ---------------------------------------------------------------------------

/// The outcome of an agent execution cycle.
///
/// Returned to the controller after the execution engine completes.
/// Contains metadata about what happened, not raw output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentResult {
    /// The goal ID that produced this result.
    pub goal_id: Uuid,
    /// Whether the execution succeeded.
    pub success: bool,
    /// Human-readable summary of the outcome.
    pub summary: String,
    /// Exit code from execution, if applicable.
    pub exit_code: Option<i32>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_is_clone() {
        let result = AgentResult {
            goal_id: Uuid::new_v4(),
            success: true,
            summary: "completed".into(),
            exit_code: Some(0),
        };
        let _cloned = result.clone();
    }
}
