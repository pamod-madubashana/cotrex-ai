use serde::Deserialize;

use crate::decision::{AgentDecision, PlanStep};

// ---------------------------------------------------------------------------
// Response Parser
// ---------------------------------------------------------------------------

/// Converts raw AI model output into validated AgentDecision.
///
/// The parser is a safety boundary. It validates structured output
/// before it reaches the capability resolver. Invalid or unknown
/// outputs are rejected.
///
/// # Format
///
/// Input must be JSON with the following structure:
///
/// ```json
/// {
///   "action": "write_file",
///   "path": "hello.txt",
///   "content": "hello"
/// }
/// ```
///
/// # Supported Actions
///
/// - `execute_command` — requires `command` field
/// - `write_file` — requires `path` and `content` fields
/// - `delete_file` — requires `path` field
/// - `complete` — optional `summary` field
/// - `need_context` — optional `summary` field
pub struct ResponseParser;

/// Errors from response parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The input is not valid JSON.
    InvalidJson(String),
    /// The action is not recognized.
    UnknownAction(String),
    /// A required field is missing.
    MissingField(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson(msg) => write!(f, "invalid JSON: {}", msg),
            Self::UnknownAction(action) => write!(f, "unknown action: {}", action),
            Self::MissingField(field) => write!(f, "missing field: {}", field),
        }
    }
}

#[derive(Deserialize)]
struct RawDecision {
    action: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    args: Option<Vec<String>>,
    #[serde(default)]
    summary: Option<String>,
}

impl ResponseParser {
    /// Parse raw model output into an AgentDecision.
    pub fn parse(raw: &str) -> Result<AgentDecision, ParseError> {
        let raw: RawDecision =
            serde_json::from_str(raw).map_err(|e| ParseError::InvalidJson(e.to_string()))?;

        match raw.action.as_str() {
            "execute_command" => {
                let command = raw
                    .command
                    .ok_or_else(|| ParseError::MissingField("command".into()))?;
                let args = raw.args.unwrap_or_default();
                Ok(AgentDecision::Execute(PlanStep::ExecuteCommand {
                    command,
                    args,
                }))
            }
            "write_file" => {
                let path = raw
                    .path
                    .ok_or_else(|| ParseError::MissingField("path".into()))?;
                let content = raw
                    .content
                    .ok_or_else(|| ParseError::MissingField("content".into()))?;
                Ok(AgentDecision::Execute(PlanStep::WriteFile {
                    path,
                    content: content.into_bytes(),
                }))
            }
            "delete_file" => {
                let path = raw
                    .path
                    .ok_or_else(|| ParseError::MissingField("path".into()))?;
                Ok(AgentDecision::Execute(PlanStep::DeleteFile { path }))
            }
            "complete" => {
                let summary = raw.summary.unwrap_or_else(|| "completed".into());
                Ok(AgentDecision::Complete(summary))
            }
            "need_context" => {
                let reason = raw.summary.unwrap_or_else(|| "need more context".into());
                Ok(AgentDecision::NeedMoreContext(reason))
            }
            other => Err(ParseError::UnknownAction(other.into())),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_write_file() {
        let json = r#"{"action":"write_file","path":"hello.txt","content":"hello"}"#;
        let decision = ResponseParser::parse(json).unwrap();
        match decision {
            AgentDecision::Execute(PlanStep::WriteFile { path, content }) => {
                assert_eq!(path, "hello.txt");
                assert_eq!(content, b"hello");
            }
            other => panic!("expected WriteFile, got: {:?}", other),
        }
    }

    #[test]
    fn parse_execute_command() {
        let json = r#"{"action":"execute_command","command":"echo","args":["hello"]}"#;
        let decision = ResponseParser::parse(json).unwrap();
        match decision {
            AgentDecision::Execute(PlanStep::ExecuteCommand { command, args }) => {
                assert_eq!(command, "echo");
                assert_eq!(args, vec!["hello"]);
            }
            other => panic!("expected ExecuteCommand, got: {:?}", other),
        }
    }

    #[test]
    fn parse_delete_file() {
        let json = r#"{"action":"delete_file","path":"test.txt"}"#;
        let decision = ResponseParser::parse(json).unwrap();
        match decision {
            AgentDecision::Execute(PlanStep::DeleteFile { path }) => {
                assert_eq!(path, "test.txt");
            }
            other => panic!("expected DeleteFile, got: {:?}", other),
        }
    }

    #[test]
    fn parse_complete() {
        let json = r#"{"action":"complete","summary":"done"}"#;
        let decision = ResponseParser::parse(json).unwrap();
        assert_eq!(decision, AgentDecision::Complete("done".into()));
    }

    #[test]
    fn parse_need_context() {
        let json = r#"{"action":"need_context","summary":"unclear goal"}"#;
        let decision = ResponseParser::parse(json).unwrap();
        assert_eq!(
            decision,
            AgentDecision::NeedMoreContext("unclear goal".into())
        );
    }

    #[test]
    fn reject_invalid_json() {
        let result = ResponseParser::parse("not json at all");
        assert!(matches!(result, Err(ParseError::InvalidJson(_))));
    }

    #[test]
    fn reject_unknown_action() {
        let json = r#"{"action":"do_magic"}"#;
        let result = ResponseParser::parse(json);
        assert!(matches!(result, Err(ParseError::UnknownAction(_))));
    }

    #[test]
    fn reject_missing_field() {
        let json = r#"{"action":"write_file"}"#;
        let result = ResponseParser::parse(json);
        assert!(matches!(result, Err(ParseError::MissingField(_))));
    }
}
