# RFC-0010: Model Output Contract

**Status:** Draft
**Milestone:** 13
**Depends on:** RFC-0007 (Local Provider Runtime), RFC-0009 (Inference Pipeline)

---

## 1. Purpose

RFC-0010 defines the output pipeline: how raw model output
becomes typed capability responses.

Before this RFC, the adapter blindly wrapped model text as
BuildSummaryResponse. After this RFC, model output is parsed
into structured format, and capability-specific extraction
happens in the capability layer.

---

## 2. Glossary

- **ModelOutput**: raw model output with parsed format
  classification.
- **OutputFormat**: enum classifying output as Json, Text, or
  Empty.
- **OutputParser**: a trait that classifies raw model output.
- **CapabilityResponseParser**: a trait that extracts typed
  responses from ModelOutput.

---

## 3. Architecture Position

The output pipeline sits between model inference and capability
response.

```text
InferenceResponse { text }
      │
      ▼
OutputParser
      │
      ▼
ModelOutput { raw, format, warnings }
      │
      ▼
CapabilityResponseParser
      │
      ▼
CapabilityResponse
```

### Ownership

**Runtime owns:**

- OutputParser trait
- ModelOutput type
- OutputFormat enum
- JSON extraction logic

**Capability layer owns:**

- CapabilityResponseParser trait (defined in runtime)
- BuildSummaryParser implementation
- ExplainRustParser implementation
- Future capability parsers

**Neither owns:**

- Model execution (provider)
- Context construction (RFC-0009)

---

## 4. ModelOutput

### OutputFormat

```rust
pub enum OutputFormat {
    Json(serde_json::Value),
    Text(String),
    Empty,
}
```

Why exhaustive:

- `Json` — model produced valid JSON
- `Text` — model produced non-JSON text
- `Empty` — model produced no output

No `Invalid` variant. Invalid states are reported through
warnings, not type system tricks.

### ModelOutput

```rust
pub struct ModelOutput {
    pub raw: String,
    pub format: OutputFormat,
    pub warnings: Vec<String>,
}
```

### Raw Output Invariant

The runtime must preserve raw model output unchanged:

```
model_output.raw == inference_response.text
```

No cleanup. No markdown removal. No trimming. No "helpful"
transformations.

The parser may interpret. It must not rewrite history.

This protects:

- Replay (re-run with identical inputs)
- Debugging (see what the model actually said)
- Parser improvements (re-parse old outputs)
- Evaluation datasets (exact model output for comparison)

---

## 5. OutputParser Trait

```rust
pub trait OutputParser {
    fn parse(&self, response: &InferenceResponse) -> ModelOutput;
}
```

### JSON Extraction Pipeline

```text
raw output
    │
    ▼
direct serde_json::from_str
    │
    ▼ (if fail)
find first '{'
    │
    ▼
streaming first-object parse
    │
    ▼ (if fail)
classify as Text or Empty
```

### Why Streaming First-Object

Local models often emit:

```text
Here is the result:

```json
{
  "success": true
}
```

Hope this helps!
```

The streaming approach:

1. Finds the first `{`
2. Uses `serde_json::Deserializer::into_iter::<Value>()`
3. Takes the first complete JSON object
4. Ignores trailing prose

This handles:

- Markdown fences
- Surrounding prose
- Multiple JSON objects (takes first)
- Weak models that add decorative text

### Empty Detection

```rust
if text.trim().is_empty() {
    OutputFormat::Empty
}
```

Empty output gets its own variant because it is semantically
different from text output.

---

## 6. CapabilityResponseParser Trait

```rust
pub trait CapabilityResponseParser {
    fn parse(
        &self,
        output: &ModelOutput,
        request: &CapabilityRequest,
    ) -> CapabilityResponse;
}
```

### Why the Request Parameter

The parser needs the original request to:

- Know which response type to produce
- Validate fields against expectations
- Provide meaningful warnings

### Why Defined in Runtime

The trait is defined in runtime because:

- Runtime knows about CapabilityRequest/CapabilityResponse
- Capability crates implement the trait
- Providers never see the trait

This keeps the dependency direction correct:

```
runtime → defines traits
capabilities → implements traits
providers → unaware of parsing
```

---

## 7. Capability Parser Examples

### BuildSummaryParser

```rust
pub struct BuildSummaryParser;

impl CapabilityResponseParser for BuildSummaryParser {
    fn parse(&self, output: &ModelOutput, _request: &CapabilityRequest) -> CapabilityResponse {
        let mut warnings = output.warnings.clone();

        match &output.format {
            OutputFormat::Json(v) => {
                let success = v.get("success")
                    .and_then(|s| s.as_bool())
                    .unwrap_or_else(|| {
                        warnings.push("missing required field: success".into());
                        false
                    });
                let summary = v.get("summary")
                    .and_then(|s| s.as_str())
                    .unwrap_or_else(|| {
                        warnings.push("missing required field: summary".into());
                        ""
                    });
                let recommendation = v.get("recommendation")
                    .and_then(|s| s.as_str())
                    .map(String::from);

                CapabilityResponse::BuildSummary(BuildSummaryResponse {
                    success,
                    summary: summary.into(),
                    recommendation,
                })
            }
            OutputFormat::Text(t) => CapabilityResponse::BuildSummary(BuildSummaryResponse {
                success: true,
                summary: t.clone(),
                recommendation: None,
            }),
            OutputFormat::Empty => CapabilityResponse::BuildSummary(BuildSummaryResponse {
                success: false,
                summary: String::new(),
                recommendation: None,
            }),
        }
    }
}
```

### Fallback Behavior

For `{}` (empty JSON object):

```rust
success = false
summary = ""
warnings = ["missing required field: summary"]
```

Empty structured output is not success. The parser produces
a warning and a failure response.

---

## 8. Orchestration Integration

The single integration point that connects everything:

```rust
pub fn execute_capability(
    provider: &dyn CapabilityProvider,
    output_parser: &dyn OutputParser,
    capability_parser: &dyn CapabilityResponseParser,
    request: CapabilityRequest,
) -> Result<CapabilityResponse, RuntimeError> {
    let response = provider.execute(request.clone())?;
    let model_output = output_parser.parse(&response);
    Ok(capability_parser.parse(&model_output, &request))
}
```

This is the only function callers should use. Manual
composition is forbidden to prevent divergent inference paths.

---

## 9. Invariants

1. `model_output.raw == inference_response.text` always.
2. OutputParser never panics on malformed output.
3. CapabilityResponseParser produces valid CapabilityResponse
   even for unexpected output (fallback, not crash).
4. Empty JSON `{}` produces failure, not success.
5. Warnings accumulate through the parsing pipeline.
6. Providers never see OutputParser or CapabilityResponseParser.
7. `execute_capability` is the single integration point.

---

## 10. Non-Goals

- Output validation beyond field extraction
- Model confidence scoring
- Token-level analysis
- Output caching
- Retry on malformed output
- Streaming response parsing

---

## 11. Exit Criteria

RFC-0010 is complete when:

- OutputParser trait and default implementation are defined.
- ModelOutput and OutputFormat types are implemented.
- CapabilityResponseParser trait is defined.
- BuildSummaryParser and ExplainRustParser are implemented.
- `execute_capability` orchestration function works.
- Raw output invariant is validated by tests.
- JSON extraction handles fences, prose, and fallback.
- `cargo fmt`, `cargo clippy -D warnings`, `cargo test` all pass.

---

**End of RFC-0010**
