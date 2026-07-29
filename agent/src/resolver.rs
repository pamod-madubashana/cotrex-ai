use std::path::PathBuf;

use execution::{Capability, ExecutionAction, ExecutionRequest};

use crate::decision::PlanStep;

// ---------------------------------------------------------------------------
// Capability Resolver
// ---------------------------------------------------------------------------

/// Translates planner decisions into execution requests.
///
/// The resolver owns the policy boundary between intent and execution.
/// It validates that a plan step maps to a known capability before
/// creating an execution request.
///
/// # Responsibility
///
/// - Convert `PlanStep` → `ExecutionRequest`
/// - Validate capability support
/// - Reject unknown capabilities
///
/// # Non-responsibility
///
/// - Reasoning about goals (planner's job)
/// - Executing commands (executor's job)
/// - Recording events (engine's job)
pub trait CapabilityResolver {
    /// Resolve a plan step into an execution request.
    fn resolve(&self, step: PlanStep) -> Result<ExecutionRequest, ResolutionError>;
}

// ---------------------------------------------------------------------------
// Default Resolver
// ---------------------------------------------------------------------------

/// Default implementation of CapabilityResolver.
///
/// Supports all three RFC-0004 capabilities:
/// - CommandRun
/// - FileWrite
/// - FileDelete
pub struct DefaultResolver;

/// Errors from capability resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionError {
    /// The capability is not supported.
    UnsupportedCapability(String),
    /// The plan step is invalid.
    InvalidPlanStep(String),
}

impl std::fmt::Display for ResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedCapability(msg) => write!(f, "unsupported capability: {}", msg),
            Self::InvalidPlanStep(msg) => write!(f, "invalid plan step: {}", msg),
        }
    }
}

impl CapabilityResolver for DefaultResolver {
    fn resolve(&self, step: PlanStep) -> Result<ExecutionRequest, ResolutionError> {
        let (action, capabilities) = match step {
            PlanStep::ExecuteCommand { command, args } => {
                // Note: Arguments are concatenated into a single command string.
                // This matches RFC-0004's ExecutionAction::CommandRun model.
                // Shell metacharacters in arguments are NOT escaped.
                // This is a known limitation for future resolution.
                let full_command = if args.is_empty() {
                    command
                } else {
                    format!("{} {}", command, args.join(" "))
                };
                (
                    ExecutionAction::CommandRun {
                        command: full_command,
                        working_directory: PathBuf::from("."),
                    },
                    vec![Capability::CommandRun],
                )
            }
            PlanStep::WriteFile { path, content } => (
                ExecutionAction::FileWrite {
                    path: PathBuf::from(path),
                    content,
                    working_directory: PathBuf::from("."),
                },
                vec![Capability::FileWrite],
            ),
            PlanStep::DeleteFile { path } => (
                ExecutionAction::FileDelete {
                    path: PathBuf::from(path),
                    working_directory: PathBuf::from("."),
                },
                vec![Capability::FileDelete],
            ),
        };

        Ok(ExecutionRequest::new(action, capabilities))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_conversion() {
        let resolver = DefaultResolver;
        let step = PlanStep::ExecuteCommand {
            command: "echo".into(),
            args: vec!["hello".into()],
        };
        let req = resolver.resolve(step).unwrap();
        assert_eq!(req.required_capabilities, vec![Capability::CommandRun]);
    }

    #[test]
    fn write_file_conversion() {
        let resolver = DefaultResolver;
        let step = PlanStep::WriteFile {
            path: "test.txt".into(),
            content: b"hello".to_vec(),
        };
        let req = resolver.resolve(step).unwrap();
        assert_eq!(req.required_capabilities, vec![Capability::FileWrite]);
    }

    #[test]
    fn delete_file_conversion() {
        let resolver = DefaultResolver;
        let step = PlanStep::DeleteFile {
            path: "test.txt".into(),
        };
        let req = resolver.resolve(step).unwrap();
        assert_eq!(req.required_capabilities, vec![Capability::FileDelete]);
    }
}
