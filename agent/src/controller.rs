use execution::{ExecutionEngine, ExecutionResult};

use crate::context::AgentContext;
use crate::decision::{AgentDecision, AgentGoal};
use crate::planner::Planner;
use crate::resolver::CapabilityResolver;
use crate::result::AgentResult;

// ---------------------------------------------------------------------------
// Agent Controller
// ---------------------------------------------------------------------------

/// Bridges planning and execution.
///
/// The controller is the only component that touches both the planner and
/// the execution engine. It uses the resolver to convert decisions into
/// requests.
///
/// # Responsibility
///
/// - Receive goals
/// - Call the planner
/// - Use resolver to convert decisions into execution requests
/// - Call the execution engine
/// - Convert results into agent results
///
/// # Non-responsibility
///
/// - Reasoning about goals (planner's job)
/// - Executing commands (executor's job)
/// - Recording events (engine's job)
/// - Interpreting output (later milestone)
/// - Creating execution requests (resolver's job)
pub struct AgentController<P> {
    planner: P,
    resolver: Box<dyn CapabilityResolver>,
    engine: ExecutionEngine,
}

impl<P: Planner> AgentController<P> {
    /// Create a new agent controller.
    pub fn new(planner: P, resolver: Box<dyn CapabilityResolver>, engine: ExecutionEngine) -> Self {
        Self {
            planner,
            resolver,
            engine,
        }
    }

    /// Process a goal through the full agent cycle.
    ///
    /// ```text
    /// AgentGoal → Planner → AgentDecision → Resolver → ExecutionRequest → ExecutionEngine → AgentResult
    /// ```
    pub fn process_goal(&self, goal: &AgentGoal) -> Result<AgentResult, AgentError> {
        // Step 1: Plan
        let decision = self.planner.plan(&goal.description);

        // Step 2: Convert decision to execution request and execute
        match decision {
            AgentDecision::Execute(plan_step) => {
                let mut request = self
                    .resolver
                    .resolve(plan_step)
                    .map_err(|e| AgentError::ExecutionFailed(e.to_string()))?;
                request.id = goal.id;
                let result = self.engine.submit(&request).map_err(|e| {
                    AgentError::ExecutionFailed(format!("execution engine error: {}", e))
                })?;
                Ok(self.result_to_agent_result(goal.id, &result))
            }
            AgentDecision::Complete(summary) => Ok(AgentResult {
                goal_id: goal.id,
                success: true,
                summary,
                exit_code: None,
            }),
            AgentDecision::NeedMoreContext(reason) => Err(AgentError::NeedMoreContext(reason)),
        }
    }

    /// Process a goal with context.
    pub fn process_goal_with_context(
        &self,
        goal: &AgentGoal,
        _context: &AgentContext,
    ) -> Result<AgentResult, AgentError> {
        // For now, context is ignored — the mock planner doesn't use it.
        // This method exists to validate the context boundary.
        self.process_goal(goal)
    }

    fn result_to_agent_result(&self, goal_id: uuid::Uuid, result: &ExecutionResult) -> AgentResult {
        let summary = if result.success {
            format!("execution completed (exit code: {:?})", result.exit_code)
        } else {
            format!(
                "execution failed: {}",
                result.error.as_deref().unwrap_or("unknown error")
            )
        };

        AgentResult {
            goal_id,
            success: result.success,
            summary,
            exit_code: result.exit_code,
        }
    }
}

// ---------------------------------------------------------------------------
// Agent Error
// ---------------------------------------------------------------------------

/// Errors specific to the agent layer.
#[derive(Debug)]
pub enum AgentError {
    /// The planner needs more information.
    NeedMoreContext(String),
    /// Execution failed.
    ExecutionFailed(String),
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NeedMoreContext(reason) => write!(f, "need more context: {}", reason),
            Self::ExecutionFailed(reason) => write!(f, "execution failed: {}", reason),
        }
    }
}

impl std::error::Error for AgentError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::MockPlanner;
    use crate::resolver::DefaultResolver;
    use execution::{ExecutionActionDiscriminant, ExecutionError, Executor};
    use execution::{ExecutionEngine, ExecutionPolicy, ExecutorRegistry};

    /// A test executor that always succeeds.
    struct SucceedExecutor;

    impl Executor for SucceedExecutor {
        fn execute(
            &self,
            request: &execution::ExecutionRequest,
        ) -> Result<ExecutionResult, ExecutionError> {
            Ok(ExecutionResult {
                execution_id: request.id,
                success: true,
                exit_code: Some(0),
                duration_ms: 0,
                error: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
    }

    fn test_controller() -> AgentController<MockPlanner> {
        let store = kernel::EventStore::new();
        let validator = Box::new(ExecutionPolicy::allow_all());
        let mut registry = ExecutorRegistry::new();
        registry
            .register(
                ExecutionActionDiscriminant::CommandRun,
                Box::new(SucceedExecutor),
            )
            .unwrap();
        registry
            .register(
                ExecutionActionDiscriminant::FileWrite,
                Box::new(SucceedExecutor),
            )
            .unwrap();
        registry
            .register(
                ExecutionActionDiscriminant::FileDelete,
                Box::new(SucceedExecutor),
            )
            .unwrap();
        let engine = ExecutionEngine::new(store, validator, registry);
        AgentController::new(MockPlanner, Box::new(DefaultResolver), engine)
    }

    #[test]
    fn create_file_end_to_end() {
        let controller = test_controller();
        let goal = AgentGoal::new("create hello.txt");
        let result = controller.process_goal(&goal).unwrap();

        assert!(result.success);
        assert_eq!(result.exit_code, Some(0));
        assert!(result.summary.contains("completed"));
    }

    #[test]
    fn delete_file_end_to_end() {
        let controller = test_controller();
        let goal = AgentGoal::new("delete test.txt");
        let result = controller.process_goal(&goal).unwrap();

        assert!(result.success);
    }

    #[test]
    fn run_command_end_to_end() {
        let controller = test_controller();
        let goal = AgentGoal::new("run echo hello");
        let result = controller.process_goal(&goal).unwrap();

        assert!(result.success);
    }

    #[test]
    fn unknown_goal_returns_error() {
        let controller = test_controller();
        let goal = AgentGoal::new("do something magical");
        let result = controller.process_goal(&goal);

        match result {
            Err(AgentError::NeedMoreContext(msg)) => {
                assert!(msg.contains("do something magical"));
            }
            other => panic!("expected NeedMoreContext, got: {:?}", other),
        }
    }

    #[test]
    fn goal_id_propagated_to_result() {
        let controller = test_controller();
        let goal = AgentGoal::new("create test.txt");
        let result = controller.process_goal(&goal).unwrap();

        assert_eq!(result.goal_id, goal.id);
    }
}
