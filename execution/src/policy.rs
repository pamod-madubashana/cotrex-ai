use crate::error::ExecutionError;
use crate::request::{Capability, ExecutionRequest};

// ---------------------------------------------------------------------------
// Capability Validator Trait (RFC-0004, Section 6)
// ---------------------------------------------------------------------------

/// Validates that an execution request has the required capabilities.
///
/// Validation is separate from execution. The engine delegates validation
/// to this trait before dispatching to an executor. If validation fails,
/// no events are appended and no action is performed.
///
/// # Responsibility
///
/// - Check that the caller holds all required capabilities
/// - Reject requests that lack necessary permissions
///
/// # Non-responsibility
///
/// - Performing the action
/// - Assigning event IDs
/// - Updating state
pub trait CapabilityValidator: Send + Sync {
    /// Validate the request's capabilities.
    ///
    /// Returns `Ok(())` if the request is allowed.
    /// Returns `Err(ExecutionError::ValidationFailed)` if rejected.
    fn validate(&self, request: &ExecutionRequest) -> Result<(), ExecutionError>;
}

// ---------------------------------------------------------------------------
// Execution Policy
// ---------------------------------------------------------------------------

/// A simple policy that maps capabilities to allowed actions.
///
/// This is a default policy implementation. More sophisticated policies
/// (allowlist, denylist, role-based) can be built by implementing
/// [`CapabilityValidator`] directly.
pub struct ExecutionPolicy {
    /// Capabilities this policy grants.
    granted: Vec<Capability>,
}

impl ExecutionPolicy {
    /// Create a new policy with the given granted capabilities.
    pub fn new(granted: Vec<Capability>) -> Self {
        Self { granted }
    }

    /// Create a policy that grants all capabilities.
    pub fn allow_all() -> Self {
        Self {
            granted: vec![
                Capability::CommandRun,
                Capability::FileWrite,
                Capability::FileDelete,
            ],
        }
    }

    /// Create a policy that grants no capabilities.
    pub fn deny_all() -> Self {
        Self { granted: vec![] }
    }
}

impl CapabilityValidator for ExecutionPolicy {
    fn validate(&self, request: &ExecutionRequest) -> Result<(), ExecutionError> {
        for required in &request.required_capabilities {
            if !self.granted.contains(required) {
                return Err(ExecutionError::ValidationFailed(format!(
                    "missing capability: {:?}",
                    required
                )));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::ExecutionAction;
    use std::path::PathBuf;

    fn command_request() -> ExecutionRequest {
        ExecutionRequest::new(
            ExecutionAction::CommandRun {
                command: "ls".into(),
                working_directory: PathBuf::from("."),
            },
            vec![Capability::CommandRun],
        )
    }

    #[test]
    fn allow_all_permits_command() {
        let policy = ExecutionPolicy::allow_all();
        assert!(policy.validate(&command_request()).is_ok());
    }

    #[test]
    fn deny_all_rejects_command() {
        let policy = ExecutionPolicy::deny_all();
        let result = policy.validate(&command_request());
        assert!(result.is_err());
    }

    #[test]
    fn partial_policy_rejects_missing() {
        let policy = ExecutionPolicy::new(vec![Capability::FileWrite]);
        let result = policy.validate(&command_request());
        assert!(result.is_err());
    }

    #[test]
    fn partial_policy_permits_granted() {
        let policy = ExecutionPolicy::new(vec![Capability::CommandRun]);
        assert!(policy.validate(&command_request()).is_ok());
    }
}
