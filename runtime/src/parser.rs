use crate::InferenceResponse;

// ---------------------------------------------------------------------------
// OutputFormat
//
// Exhaustive enum classifying model output format.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    Json(serde_json::Value),
    Text(String),
    Empty,
}

// ---------------------------------------------------------------------------
// ModelOutput
//
// Raw model output with parsed format classification. The raw
// field must always equal the original InferenceResponse.text.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ModelOutput {
    pub raw: String,
    pub format: OutputFormat,
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// OutputParser
//
// Trait that classifies raw model output.
// ---------------------------------------------------------------------------

pub trait OutputParser {
    fn parse(&self, response: &InferenceResponse) -> ModelOutput;
}

// ---------------------------------------------------------------------------
// DefaultOutputParser
//
// Default implementation that extracts JSON or falls back to text.
// ---------------------------------------------------------------------------

pub struct DefaultOutputParser;

impl OutputParser for DefaultOutputParser {
    fn parse(&self, response: &InferenceResponse) -> ModelOutput {
        let raw = response.text.clone();
        let mut warnings = Vec::new();

        let format = extract_format(&raw, &mut warnings);

        ModelOutput {
            raw,
            format,
            warnings,
        }
    }
}

// ---------------------------------------------------------------------------
// JSON extraction pipeline
//
// 1. Direct parse
// 2. Find first '{', streaming first-object parse
// 3. Classify as Text or Empty
// ---------------------------------------------------------------------------

fn extract_format(text: &str, warnings: &mut Vec<String>) -> OutputFormat {
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return OutputFormat::Empty;
    }

    // Try direct parse
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return OutputFormat::Json(v);
    }

    // Try finding first '{' and streaming first-object parse
    if let Some(start) = trimmed.find('{') {
        let slice = &trimmed[start..];
        let deserializer = serde_json::Deserializer::from_str(slice);
        if let Some(Ok(v)) = deserializer.into_iter::<serde_json::Value>().next() {
            return OutputFormat::Json(v);
        }
        warnings.push("found '{' but failed to parse JSON object".into());
    }

    // Check for '{' without matching '}'
    if trimmed.contains('{') && !trimmed.contains('}') {
        warnings.push("incomplete JSON: found '{' without matching '}'".into());
    }

    OutputFormat::Text(trimmed.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_clean_json() {
        let parser = DefaultOutputParser;
        let response = InferenceResponse {
            text: r#"{"success": true, "summary": "ok"}"#.into(),
            profile: None,
        };
        let output = parser.parse(&response);
        assert_eq!(output.raw, response.text);
        assert!(matches!(output.format, OutputFormat::Json(_)));
        assert!(output.warnings.is_empty());
    }

    #[test]
    fn parse_json_in_fences() {
        let parser = DefaultOutputParser;
        let response = InferenceResponse {
            text: "```json\n{\"success\": true}\n```".into(),
            profile: None,
        };
        let output = parser.parse(&response);
        assert!(matches!(output.format, OutputFormat::Json(_)));
    }

    #[test]
    fn parse_json_surrounded_by_prose() {
        let parser = DefaultOutputParser;
        let response = InferenceResponse {
            text: "Here is the result:\n{\"success\": true}\nHope this helps!".into(),
            profile: None,
        };
        let output = parser.parse(&response);
        assert!(matches!(output.format, OutputFormat::Json(_)));
    }

    #[test]
    fn parse_plain_text() {
        let parser = DefaultOutputParser;
        let response = InferenceResponse {
            text: "The build failed because of a missing dependency.".into(),
            profile: None,
        };
        let output = parser.parse(&response);
        assert!(matches!(output.format, OutputFormat::Text(_)));
        assert!(output.warnings.is_empty());
    }

    #[test]
    fn parse_empty_output() {
        let parser = DefaultOutputParser;
        let response = InferenceResponse {
            text: String::new(),
            profile: None,
        };
        let output = parser.parse(&response);
        assert_eq!(output.format, OutputFormat::Empty);
    }

    #[test]
    fn parse_whitespace_only() {
        let parser = DefaultOutputParser;
        let response = InferenceResponse {
            text: "   \n  \t  ".into(),
            profile: None,
        };
        let output = parser.parse(&response);
        assert_eq!(output.format, OutputFormat::Empty);
    }

    #[test]
    fn raw_preserved_exactly() {
        let parser = DefaultOutputParser;
        let original = "  ```json\n{\"a\": 1}\n```  ";
        let response = InferenceResponse {
            text: original.into(),
            profile: None,
        };
        let output = parser.parse(&response);
        assert_eq!(output.raw, original);
    }

    #[test]
    fn incomplete_json_gets_warning() {
        let parser = DefaultOutputParser;
        let response = InferenceResponse {
            text: "something { incomplete".into(),
            profile: None,
        };
        let output = parser.parse(&response);
        assert!(!output.warnings.is_empty());
    }

    #[test]
    fn first_object_extracted() {
        let parser = DefaultOutputParser;
        let response = InferenceResponse {
            text: r#"{"a": 1} {"b": 2}"#.into(),
            profile: None,
        };
        let output = parser.parse(&response);
        match output.format {
            OutputFormat::Json(v) => {
                assert_eq!(v.get("a").and_then(|x| x.as_i64()), Some(1));
                assert!(v.get("b").is_none()); // only first object
            }
            _ => panic!("expected JSON"),
        }
    }
}
