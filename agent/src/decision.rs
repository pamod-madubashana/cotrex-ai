use uuid::Uuid;

// ---------------------------------------------------------------------------
// Agent Goal
// ---------------------------------------------------------------------------

/// Represents user intent submitted to the agent layer.
///
/// A goal is the starting point of the agent reasoning cycle.
/// The planner converts goals into plans, which become execution requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentGoal {
    /// Unique identifier for this goal.
    pub id: Uuid,
    /// Human-readable description of what the user wants.
    pub description: String,
}

impl AgentGoal {
    /// Create a new agent goal with a generated UUID.
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            description: description.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Plan Step
// ---------------------------------------------------------------------------

/// Represents an intended capability action produced by the planner.
///
/// Each step maps to a specific execution domain. The controller translates
/// steps into `ExecutionRequest` values for the `ExecutionEngine`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanStep {
    /// Execute a shell command.
    ExecuteCommand {
        /// The command to run.
        command: String,
        /// Arguments to pass to the command.
        args: Vec<String>,
    },
    /// Write content to a file.
    WriteFile {
        /// Target file path (relative to working directory).
        path: String,
        /// Content to write.
        content: Vec<u8>,
    },
    /// Delete a file.
    DeleteFile {
        /// Target file path (relative to working directory).
        path: String,
    },
}

// ---------------------------------------------------------------------------
// Agent Plan
// ---------------------------------------------------------------------------

/// Represents planner output: an ordered sequence of steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentPlan {
    /// The steps to execute in order.
    pub steps: Vec<PlanStep>,
}

// ---------------------------------------------------------------------------
// Agent Decision
// ---------------------------------------------------------------------------

/// Represents the planner's decision for a given goal.
///
/// The planner produces decisions; the controller translates them into
/// execution requests. The planner never touches the execution layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentDecision {
    /// Execute a single plan step.
    Execute(PlanStep),
    /// The goal has been achieved.
    Complete(String),
    /// The planner needs more information to proceed.
    NeedMoreContext(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_goal_new_generates_id() {
        let goal = AgentGoal::new("test goal");
        assert_eq!(goal.description, "test goal");
        // UUID is non-zero
        assert_ne!(goal.id, Uuid::nil());
    }

    #[test]
    fn plan_step_is_clone() {
        let step = PlanStep::ExecuteCommand {
            command: "echo".into(),
            args: vec!["hello".into()],
        };
        let _cloned = step.clone();
    }

    #[test]
    fn agent_decision_is_clone() {
        let decision = AgentDecision::Complete("done".into());
        let _cloned = decision;
    }
}
