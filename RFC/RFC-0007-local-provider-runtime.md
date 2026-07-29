# RFC-0007: Local Provider Runtime

**Status:** Accepted
**Milestone:** 11
**Depends on:** RFC-0005 (AI Runtime Integration)

---

## 1. Purpose

This RFC defines the Local Provider Runtime: the infrastructure for
loading, managing, and executing inference engines behind the existing
`CapabilityProvider` trait.

RFC-0007 establishes the runtime architecture independent of any
specific backend. It defines lifecycle management, model abstraction,
configuration, and the boundary between prompt construction and
inference execution.

The `CapabilityProvider` trait remains unchanged. Providers remain
replaceable. Models are implementation details. This RFC describes the
machinery that makes real inference possible.

---

## 2. Glossary

- **LocalModel**: a trait that abstracts model loading, inference,
  and unloading behind a typed interface.
- **ProviderState**: the observable lifecycle state of a provider's
  inference engine.
- **InferenceRequest**: a typed request containing prompt and
  generation parameters.
- **InferenceResponse**: a typed response containing generated text.
- **ContextBuilder**: the subsystem that constructs prompts from
  project state (events, projections, workspace graph).
- **CapabilityProvider**: the protocol-level trait that Cotrex talks to.
  Wraps a `LocalModel` and handles protocol translation.

---

## 3. Architecture Position

The Local Provider Runtime sits between the cotrex-ai Runtime and
inference backends. It translates protocol requests into typed
inference calls.

```text
                 Intelligence Brain
                        │
                        ▼
                Context Builder
                        │
                        ▼
                  Prompt / InferenceRequest
                        │
                        ▼
               CapabilityProvider
                        │
                        ▼
                  LocalModel
                        │
            ┌───────────┼───────────┐
            ▼           ▼           ▼
        llama.cpp     Candle     ONNX
```

### Provider Layer

The provider is what Cotrex talks to. It implements
`CapabilityProvider` and handles:

- Protocol translation (`CapabilityRequest` → `InferenceRequest`)
- Engine lifecycle management
- State transitions
- Error handling

### Model Layer

The model is an implementation detail. It implements `LocalModel`
and handles:

- Model loading and unloading
- Inference execution
- Resource management (GPU memory, threads)
- Internal synchronization

The provider owns the model. The model never escapes the provider
boundary.

### Ownership Rules

**Intelligence Brain owns:**

- Context construction (events, projections, workspace graph)
- Prompt assembly
- Deciding when to invoke capabilities

**Provider owns:**

- Protocol translation
- Model lifecycle management
- State transitions
- Error handling

**Model owns:**

- Model loading and unloading
- Inference execution
- Resource management
- Internal synchronization

**The provider must never:**

- Construct prompts
- Access the Event Store directly
- Know where prompts came from
- Expose backend-specific APIs upward

---

## 4. Provider State Machine

Every provider manages a `ProviderState` that tracks the inference
engine lifecycle.

### States

```text
Uninitialized
      │
      ▼
Loading
      │
 ┌────┴────┐
 │         │
 ▼         ▼
Ready    Failed
 │
 ▼
Unloading
 │
 ▼
Uninitialized
```

| State | Description |
|-------|-------------|
| Uninitialized | No model loaded. Initial state. |
| Loading | Model is being loaded into memory. |
| Ready | Model loaded and available for inference. |
| Failed | Loading failed. Observable, not terminal. |
| Unloading | Model is being released from memory. |

### Allowed Transitions

| From | To | Trigger |
|------|----|---------|
| Uninitialized | Loading | `load()` called |
| Loading | Ready | Model loaded successfully |
| Loading | Failed | Loading failed (model missing, OOM, corrupted) |
| Ready | Unloading | `unload()` called |
| Failed | Loading | `load()` called again (retry) |
| Unloading | Uninitialized | Model unloaded |

### Invalid Transitions

- Uninitialized → Ready (must load first)
- Uninitialized → Busy (must load first)
- Ready → Loading (must unload first)
- Failed → Ready (must reload)
- Failed → Busy (must reload)
- Unloading → anything except Uninitialized

### Key Rule

> Failed is an observable state, not a dead state.

Recovery is simply another `load()` attempt. The provider reports
failure through `health()`; the caller decides whether to retry.

---

## 5. LocalModel Trait

The `LocalModel` trait abstracts model execution independent of
backend.

```rust
pub trait LocalModel: Send + Sync {
    /// Returns the current provider state.
    fn state(&self) -> ProviderState;

    /// Loads the model into memory.
    ///
    /// Transitions: Uninitialized → Loading → Ready or Failed
    ///              Failed → Loading → Ready or Failed
    fn load(&mut self) -> Result<(), ProviderError>;

    /// Executes inference on the loaded model.
    ///
    /// Transitions: Ready (no state change)
    fn infer(&self, request: InferenceRequest) -> Result<InferenceResponse, ProviderError>;

    /// Unloads the model from memory.
    ///
    /// Transitions: Ready → Unloading → Uninitialized
    fn unload(&mut self) -> Result<(), ProviderError>;

    /// Returns model metadata.
    fn info(&self) -> ModelInfo;
}
```

### Why unload() Exists

GPU memory is an explicit resource. Dropping an object is not the
same as intentionally unloading a 12 GB model from VRAM. `unload()`
gives the caller explicit control over resource release.

### Thread Safety

`LocalModel` requires `Send + Sync`. Implementations must handle
internal synchronization through `Mutex`, `RwLock`, channels, or
worker threads. Consumers should never care about synchronization
strategy.

---

## 6. Inference Request/Response

Providers never receive raw strings. They receive typed structures
that can evolve without breaking the trait.

### InferenceRequest

```rust
pub struct InferenceRequest {
    pub prompt: String,
    pub temperature: f32,
    pub max_tokens: u32,
}
```

### InferenceResponse

```rust
pub struct InferenceResponse {
    pub text: String,
}
```

### Why Typed?

- Parameters evolve without trait changes
- Validation happens at construction, not inside the engine
- Different engines can optimize for known parameter ranges
- Future extensions (stop sequences, top_p, seed) don't break callers

---

## 7. Configuration

Configuration is split into global and project levels, following the
Git model.

### Global Config

Location: `~/.config/cotrex/config.toml`

Contains machine-level defaults that apply across all projects.

```toml
[provider]
backend = "llama.cpp"

[engine]
threads = 8
gpu_layers = 35
```

### Project Config

Location: `cotrex.toml` (project root)

Contains repository-specific behavior that overrides global defaults.

```toml
[model]
name = "qwen2.5-coder"
context = 8192
temperature = 0.1
max_tokens = 2048
```

### Merge Rules

- Project config overrides global config
- Missing values fall back to global defaults
- Missing global values fall back to compiled defaults
- Configuration is read once at provider initialization
- Hot-reloading is not part of this RFC

### Why Split?

- Machine settings (threads, GPU layers) differ per workstation
- Project settings (model, temperature) differ per repository
- Developers check in `cotrex.toml`; global config stays local
- Exactly like `.gitconfig` vs `.git/config`

---

## 8. Model Lifecycle

Load exactly once. Infer many times. Unload when done.

### Lifecycle

```text
startup
   │
   ▼
load model
   │
   ▼
keep in memory
   │
   ▼
infer
   │
   ▼
infer
   │
   ▼
infer
   │
   ▼
shutdown
   │
   ▼
unload model
```

### What Must Not Happen

```text
request
   │
   ▼
load model      ← never do this
   │
   ▼
infer
   │
   ▼
free model      ← never do this
```

Loading per-request would make every inference feel like booting an
operating system to open Calculator.

### Configuration Immutability

Configuration is fixed after `load()` succeeds. Changing:

- model path
- context size
- GPU layers
- thread count

requires unloading and loading again. This keeps runtime behavior
deterministic.

---

## 9. Context Builder

Prompt construction belongs to the Intelligence Brain, not the
provider. The provider receives a ready-to-infer prompt.

### Ownership

```text
Kernel
    │
    ▼
Event Store ──▶ Projection Engine ──▶ Workspace Graph
                                          │
                                          ▼
                                  Execution Context
                                          │
                                          ▼
                              Intelligence Brain
                                          │
                                          ▼
                                  Context Builder
                                          │
                                          ▼
                                  InferenceRequest
                                          │
                                          ▼
                                      LocalModel
```

### Responsibility

The Context Builder:

- Reads events from the Event Store
- Queries projections for derived state
- Assembles workspace context (file structure, recent changes)
- Constructs the prompt
- Returns an `InferenceRequest`

The Context Builder does NOT:

- Know which engine is being used
- Know about GGUF, safetensors, or ONNX
- Handle model loading or inference
- Manage GPU resources

### Separation

The provider should never know where prompts came from. It receives:

```text
InferenceRequest
```

Nothing more.

---

## 10. Invariants

Every conforming implementation MUST satisfy:

1. The `CapabilityProvider` trait is unchanged.
2. Providers remain replaceable without runtime changes.
3. `LocalModel` is `Send + Sync`.
4. The provider does not construct prompts.
5. The provider does not access the Event Store directly.
6. `Failed` state is observable, not terminal.
7. `unload()` explicitly releases resources.
8. Configuration is read once at initialization.
9. `InferenceRequest` and `InferenceResponse` are typed structs.
10. Backend details never leak above the provider boundary.
11. A `CapabilityProvider` owns exactly one `LocalModel` instance
    during its lifetime.
12. Inference must not mutate model configuration. Changing model
    path, context size, GPU layers, or thread count requires
    unloading and loading again.
13. `LocalModel` implementations must produce deterministic behavior
    for identical configuration and identical inference input. The
    runtime must not introduce hidden state between requests.

---

## 11. Future Backend Compatibility

RFC-0007 defines the abstraction layer required for local AI
execution.

The architecture is intentionally backend-agnostic. Any local
inference engine that implements the `LocalModel` trait may be
integrated without changes to:

- `CapabilityProvider`
- Runtime protocol
- AI Runtime Client
- Cotrex Core
- Intelligence Brain

Expected backends include, but are not limited to:

- llama.cpp
- Candle
- ONNX Runtime
- MLX
- Future native Cotrex runtimes

Backend-specific optimizations remain implementation details and must
not alter the runtime protocol.

---

## 12. Non-Goals

The following are explicitly out of scope for this RFC:

- Specific backend implementations (llama.cpp, Candle, ONNX)
- Model download or distribution
- Streaming token generation
- Conversation memory or chat history
- Tool calling inside the provider
- Automatic model routing
- Cloud provider fallback
- Prompt templates
- Multiple simultaneous providers
- Hot-reloading configuration
- Hot-swapping models at runtime
- Benchmarking or performance optimization

---

## 13. Exit Criteria

RFC-0007 is complete when:

- `ProviderState` is implemented and fully tested.
- `LocalModel` trait is defined and documented.
- `InferenceRequest` and `InferenceResponse` replace raw inference
  parameters.
- Configuration loading (global + project) is implemented.
- Provider lifecycle (`load → infer → unload`) is validated by tests.
- Mock implementations compile against the new interfaces.
- No backend-specific code exists in `contract` or `runtime`.
- No changes are required in Cotrex Core.
- `cargo fmt`, `cargo check`, `cargo test`, and
  `cargo clippy -D warnings` all pass.

---

## 14. Implementation Path

### Phase 1: Types and Trait

- `ProviderState` enum
- `ProviderError` enum
- `ModelInfo` struct
- `InferenceRequest` struct
- `InferenceResponse` struct
- `LocalModel` trait
- Unit tests for type construction

### Phase 2: State Machine

- State transition validation
- `load()` / `unload()` / `infer()` lifecycle
- Failed → Loading recovery
- Thread-safe state management

### Phase 3: Configuration

- Global config deserialization
- Project config deserialization
- Merge logic
- Compiled defaults

### Phase 4: Provider Wrapper

- `LocalProvider` implementing `CapabilityProvider`
- `CapabilityRequest` → `InferenceRequest` translation
- `InferenceResponse` → `CapabilityResponse` translation
- Health reporting based on engine state

### Phase 5: Context Builder

- `ContextBuilder` trait
- Event Store integration
- Projection queries
- Prompt assembly

---

**End of RFC-0007**
