use crate::decision::{AgentDecision, PlanStep};

// ---------------------------------------------------------------------------
// Planner Trait
// ---------------------------------------------------------------------------

/// Converts goals into decisions.
///
/// The planner is the reasoning core. It produces intentions only — it
/// must never touch the execution layer directly.
pub trait Planner {
    /// Produce a decision for the given goal description.
    fn plan(&self, goal: &str) -> AgentDecision;
}

// ---------------------------------------------------------------------------
// Mock Planner
// ---------------------------------------------------------------------------

/// Deterministic planner for architecture validation.
///
/// No LLM. No randomness. Same input always produces same output.
/// Exists only to validate the agent → execution boundary.
pub struct MockPlanner;

impl Planner for MockPlanner {
    fn plan(&self, goal: &str) -> AgentDecision {
        let goal_lower = goal.to_lowercase();

        // Pattern: "create <filename>"
        if let Some(filename) = goal_lower.strip_prefix("create ") {
            let filename = filename.trim();
            if !filename.is_empty() {
                return AgentDecision::Execute(PlanStep::WriteFile {
                    path: filename.into(),
                    content: format!("content of {}", filename).into_bytes(),
                });
            }
        }

        // Pattern: "delete <filename>"
        if let Some(filename) = goal_lower.strip_prefix("delete ") {
            let filename = filename.trim();
            if !filename.is_empty() {
                return AgentDecision::Execute(PlanStep::DeleteFile {
                    path: filename.into(),
                });
            }
        }

        // Pattern: "run <command>" or "execute <command>"
        if let Some(cmd) = goal_lower
            .strip_prefix("run ")
            .or_else(|| goal_lower.strip_prefix("execute "))
        {
            let cmd = cmd.trim();
            if !cmd.is_empty() {
                return AgentDecision::Execute(PlanStep::ExecuteCommand {
                    command: cmd.into(),
                    args: Vec::new(),
                });
            }
        }

        // Unknown goal
        AgentDecision::NeedMoreContext(format!(
            "I don't understand '{}'. Try: create <file>, delete <file>, run <command>",
            goal
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
    fn create_file_pattern() {
        let planner = MockPlanner;
        let decision = planner.plan("create hello.txt");

        match decision {
            AgentDecision::Execute(PlanStep::WriteFile { path, content }) => {
                assert_eq!(path, "hello.txt");
                assert!(!content.is_empty());
            }
            other => panic!("expected WriteFile, got: {:?}", other),
        }
    }

    #[test]
    fn delete_file_pattern() {
        let planner = MockPlanner;
        let decision = planner.plan("delete test.txt");

        match decision {
            AgentDecision::Execute(PlanStep::DeleteFile { path }) => {
                assert_eq!(path, "test.txt");
            }
            other => panic!("expected DeleteFile, got: {:?}", other),
        }
    }

    #[test]
    fn run_command_pattern() {
        let planner = MockPlanner;
        let decision = planner.plan("run echo hello");

        match decision {
            AgentDecision::Execute(PlanStep::ExecuteCommand { command, args }) => {
                assert_eq!(command, "echo hello");
                assert!(args.is_empty());
            }
            other => panic!("expected ExecuteCommand, got: {:?}", other),
        }
    }

    #[test]
    fn unknown_goal_returns_need_context() {
        let planner = MockPlanner;
        let decision = planner.plan("do something magical");

        match decision {
            AgentDecision::NeedMoreContext(msg) => {
                assert!(msg.contains("do something magical"));
            }
            other => panic!("expected NeedMoreContext, got: {:?}", other),
        }
    }

    #[test]
    fn deterministic_output() {
        let planner = MockPlanner;
        let d1 = planner.plan("create a.txt");
        let d2 = planner.plan("create a.txt");
        assert_eq!(d1, d2);
    }

    #[test]
    fn case_insensitive() {
        let planner = MockPlanner;
        let d1 = planner.plan("Create hello.txt");
        let d2 = planner.plan("create hello.txt");
        assert_eq!(d1, d2);
    }
}
