use crate::context::InferenceContext;
use contract::CapabilityRequest;

// ---------------------------------------------------------------------------
// PromptAssembler
//
// Trait that combines InferenceContext with raw capability data
// into prompt text.
// ---------------------------------------------------------------------------

pub trait PromptAssembler {
    fn assemble(&self, context: &InferenceContext, data: &CapabilityRequest) -> String;
}

// ---------------------------------------------------------------------------
// DefaultPromptAssembler
//
// Default implementation that produces a structured prompt from
// context and capability data.
// ---------------------------------------------------------------------------

pub struct DefaultPromptAssembler;

impl PromptAssembler for DefaultPromptAssembler {
    fn assemble(&self, context: &InferenceContext, data: &CapabilityRequest) -> String {
        let mut prompt = String::new();

        // Context header
        prompt.push_str("## Workspace Context\n\n");
        prompt.push_str(&format!("Status: {:?}\n", context.workspace_status));
        prompt.push_str(&format!("Files tracked: {}\n", context.file_count));
        prompt.push_str(&format!("Context hash: {:016x}\n\n", context.hash));

        if !context.recent_changes.is_empty() {
            prompt.push_str("Recent changes:\n");
            for change in &context.recent_changes {
                prompt.push_str(&format!("- {}\n", change));
            }
            prompt.push('\n');
        }

        // Capability-specific data
        match data {
            CapabilityRequest::BuildSummary(req) => {
                prompt.push_str("## Build Summary Request\n\n");
                prompt.push_str(&format!("Command: {}\n", req.command));
                prompt.push_str(&format!("Exit code: {}\n", req.exit_code));
                if !req.stdout.is_empty() {
                    prompt.push_str(&format!("Stdout:\n```\n{}\n```\n", req.stdout));
                }
                if !req.stderr.is_empty() {
                    prompt.push_str(&format!("Stderr:\n```\n{}\n```\n", req.stderr));
                }
                prompt.push_str("\nProvide a concise summary of this build output.");
            }
            CapabilityRequest::ExplainRust(req) => {
                prompt.push_str("## Rust Code Explanation Request\n\n");
                prompt.push_str(&format!("Question: {}\n\n", req.question));
                prompt.push_str(&format!("Source:\n```rust\n{}\n```\n", req.source));
                prompt.push_str("\nExplain the code above.");
            }
        }

        prompt
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::WorkspaceStatus;
    use contract::{BuildSummaryRequest, ExplainRustRequest, RequestMetadata};

    fn test_context() -> InferenceContext {
        InferenceContext {
            recent_changes: vec!["main.rs".into(), "lib.rs".into()],
            workspace_status: WorkspaceStatus::Modified,
            file_count: 5,
            hash: 0xdeadbeef,
        }
    }

    #[test]
    fn assemble_build_summary() {
        let assembler = DefaultPromptAssembler;
        let context = test_context();
        let request = CapabilityRequest::BuildSummary(BuildSummaryRequest {
            metadata: RequestMetadata::new(),
            command: "cargo build".into(),
            exit_code: 1,
            stdout: String::new(),
            stderr: "error: could not compile".into(),
            prompt: "unused".into(),
            temperature: 0.1,
            max_tokens: 512,
        });

        let prompt = assembler.assemble(&context, &request);
        assert!(prompt.contains("Workspace Context"));
        assert!(prompt.contains("Modified"));
        assert!(prompt.contains("cargo build"));
        assert!(prompt.contains("error: could not compile"));
    }

    #[test]
    fn assemble_explain_rust() {
        let assembler = DefaultPromptAssembler;
        let context = test_context();
        let request = CapabilityRequest::ExplainRust(ExplainRustRequest {
            metadata: RequestMetadata::new(),
            source: "fn main() {}".into(),
            question: "what does this do?".into(),
            prompt: "unused".into(),
            temperature: 0.2,
            max_tokens: 1024,
        });

        let prompt = assembler.assemble(&context, &request);
        assert!(prompt.contains("Rust Code Explanation"));
        assert!(prompt.contains("what does this do?"));
        assert!(prompt.contains("fn main() {}"));
    }

    #[test]
    fn assembler_produces_nonempty_prompt() {
        let assembler = DefaultPromptAssembler;
        let context = InferenceContext {
            recent_changes: vec![],
            workspace_status: WorkspaceStatus::Clean,
            file_count: 0,
            hash: 0,
        };
        let request = CapabilityRequest::BuildSummary(BuildSummaryRequest {
            metadata: RequestMetadata::new(),
            command: "test".into(),
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            prompt: String::new(),
            temperature: 0.1,
            max_tokens: 100,
        });

        let prompt = assembler.assemble(&context, &request);
        assert!(!prompt.is_empty());
    }

    #[test]
    fn assembler_includes_recent_changes() {
        let assembler = DefaultPromptAssembler;
        let context = InferenceContext {
            recent_changes: vec!["src/main.rs".into()],
            workspace_status: WorkspaceStatus::Modified,
            file_count: 1,
            hash: 0,
        };
        let request = CapabilityRequest::BuildSummary(BuildSummaryRequest {
            metadata: RequestMetadata::new(),
            command: "test".into(),
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            prompt: String::new(),
            temperature: 0.1,
            max_tokens: 100,
        });

        let prompt = assembler.assemble(&context, &request);
        assert!(prompt.contains("src/main.rs"));
    }
}
