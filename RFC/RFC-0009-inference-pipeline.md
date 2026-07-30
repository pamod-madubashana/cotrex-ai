# RFC-0009: Inference Pipeline

**Status:** Draft
**Milestone:** 13
**Depends on:** RFC-0007 (Local Provider Runtime), RFC-0008 (llama.cpp Provider)

---

## 1. Purpose

RFC-0009 defines the input pipeline: how kernel state becomes
inference context, how context becomes prompts, and how prompts
reach the model.

Before this RFC, the Intelligence Brain existed only as an
architectural placeholder. After this RFC, there is a concrete
data flow from kernel projections to model invocation.

---

## 2. Glossary

- **InferenceContext**: structured data extracted from kernel
  projections, ready for prompt assembly.
- **ContextBuilder**: a trait that reads projection state and
  produces an InferenceContext.
- **PromptAssembler**: a trait that combines InferenceContext
  with raw capability data into prompt text.
- **WorkspaceStatus**: an enum describing the workspace state.

---

## 3. Architecture Position

The inference pipeline sits between kernel projections and the
runtime provider layer.

```text
Kernel Events
      │
      ▼
AiContextProjection
      │
      ▼
ContextBuilder
      │
      ▼
InferenceContext
      │
      ▼
PromptAssembler
      │
      ▼
CapabilityRequest { prompt, temperature, max_tokens }
      │
      ▼
CapabilityProvider::execute()
      │
      ▼
InferenceResponse
```

### Ownership

**ContextBuilder owns:**

- Reading projection state
- Extracting relevant fields
- Computing deterministic hash
- Producing InferenceContext

**PromptAssembler owns:**

- Combining context with capability data
- Deciding prompt structure
- Formatting for the target model

**Neither owns:**

- Model selection (runtime/provider)
- Output parsing (RFC-0010)
- Event storage (kernel)

---

## 4. InferenceContext

The structured data that flows from projections to prompts.

### WorkspaceStatus

```rust
pub enum WorkspaceStatus {
    Clean,
    Modified,
    Failed,
    Unknown,
}
```

Why enum, not string:

- Deterministic serialization
- Safe hashing
- Future schema evolution
- Compiler-enforced exhaustiveness

### InferenceContext

```rust
pub struct InferenceContext {
    pub recent_changes: Vec<String>,
    pub workspace_status: WorkspaceStatus,
    pub file_count: usize,
    pub hash: u64,
}
```

### Hash Determinism

The hash must be deterministic for identical inputs:

```rust
hash(
    projection_version,
    sorted_recent_changes,  // sort to ensure ordering independence
    workspace_status as u8,
    file_count
)
```

**Critical rule:** Never hash raw Vec ordering unless ordering
is semantically meaningful. Sorting ensures that:

```
["main.rs", "lib.rs"]
```

and:

```
["lib.rs", "main.rs"]
```

produce the same hash when they represent the same state.

### Context Determinism

Same projection state + same request = same InferenceContext.

This becomes valuable for:

- Debugging (reproduce exact context)
- Replay (re-run with identical inputs)
- Caching (skip inference for unchanged context)
- Evaluation (compare model outputs on same context)

---

## 5. ContextBuilder Trait

```rust
pub trait ContextBuilder {
    fn build_context(&self, summary: &AiContextSummary) -> InferenceContext;
}
```

### Implementation

The default ContextBuilder extracts fields from
AiContextSummary and computes the hash.

```rust
pub struct DefaultContextBuilder;

impl ContextBuilder for DefaultContextBuilder {
    fn build_context(&self, summary: &AiContextSummary) -> InferenceContext {
        let mut changes = summary.recent_changes.clone();
        changes.sort();

        let status = match summary.workspace_status {
            WorkspaceStatusRaw::Empty => WorkspaceStatus::Unknown,
            WorkspaceStatusRaw::Active => WorkspaceStatus::Modified,
            WorkspaceStatusRaw::Idle => WorkspaceStatus::Clean,
        };

        let hash = compute_hash(&changes, status, summary.file_count);

        InferenceContext {
            recent_changes: changes,
            workspace_status: status,
            file_count: summary.file_count,
            hash,
        }
    }
}
```

---

## 6. PromptAssembler Trait

```rust
pub trait PromptAssembler {
    fn assemble(&self, context: &InferenceContext, data: &CapabilityRequest) -> String;
}
```

### Responsibility

The assembler receives:

- InferenceContext (structured workspace state)
- CapabilityRequest (raw capability data: command, stdout, etc.)

And produces a prompt string ready for inference.

### What the assembler does NOT do

- Call the model
- Parse model output
- Know which model is being used
- Access the Event Store directly

---

## 7. Invariants

1. Same projection state + same request = same hash.
2. ContextBuilder reads projections, never raw events.
3. PromptAssembler receives structured data, not raw strings.
4. InferenceContext is a value type (Clone, Debug).
5. No backend-specific code in context or assembler.
6. Hash sorts recent changes before computation.

---

## 8. Non-Goals

- Model selection or routing
- Output parsing (RFC-0010)
- Streaming prompt construction
- Conversation memory
- Prompt templates for specific models
- Caching or memoization

---

## 9. Exit Criteria

RFC-0009 is complete when:

- InferenceContext and WorkspaceStatus are implemented.
- ContextBuilder trait is defined with default implementation.
- PromptAssembler trait is defined.
- Hash determinism is validated by tests.
- No changes to provider implementation.
- `cargo fmt`, `cargo clippy -D warnings`, `cargo test` all pass.

---

**End of RFC-0009**
