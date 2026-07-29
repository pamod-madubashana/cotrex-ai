# RFC-0008: llama.cpp Provider

**Status:** Draft
**Milestone:** 12
**Depends on:** RFC-0007 (Local Provider Runtime)

---

## 1. Purpose

RFC-0008 implements the first real model backend for Cotrex: a
llama.cpp provider that exercises the full `LocalModel` architecture
end-to-end.

This validates that the abstraction layer defined in RFC-0007 is
sufficient for real inference engines, not just mock implementations.

The provider is isolated entirely to `providers/llama-cpp/`. No
changes to contract, runtime, kernel, execution, or agent.

---

## 2. Glossary

- **GGUF**: the file format used by llama.cpp for model weights,
  metadata, and tokenizer data.
- **Inference session**: a short-lived context created per inference
  call, used to execute the model, then destroyed.
- **LoadedConfig**: an internal subset of `ResolvedConfig` containing
  only the fields the llama.cpp backend needs.

---

## 3. Architecture Position

The llama.cpp provider sits in the provider layer, below the
runtime, behind the `LocalModel` trait.

```text
contract
        ↓
runtime (LocalModel, LocalProvider)
        ↓
providers/llama-cpp (LlamaCppModel)
        ↓
llama.cpp (native library)
```

### Boundary

**In scope:**

- `providers/llama-cpp/` crate
- Workspace member addition
- `LocalModel` implementation

**Out of scope (zero changes):**

- `contract/` — no protocol changes
- `runtime/` — no trait or adapter changes
- `kernel/` — no event store changes
- `execution/` — no executor changes
- `agent/` — no agent changes
- `adapter.rs` — response mapping is pre-existing

---

## 4. Design Decisions

### 4.1 Stateless Inference Sessions

Each `infer()` call creates a fresh inference session, executes
prompt processing, collects generated output, and destroys the
session.

```text
infer(request)
    ↓
prepare inference session
    ↓
execute inference
    ↓
collect generated output
    ↓
destroy inference session
    ↓
return InferenceResponse
```

This is intentionally slower than persistent sessions. The trade-offs:

| Property | Persistent Session | Stateless Session |
|----------|-------------------|-------------------|
| Speed | Faster (reuse) | Slower (create/destroy) |
| Thread safety | Requires locking | Inherently safe |
| Hidden state | Risk of stale state | None |
| Determinism | Harder to guarantee | Guaranteed |

Session reuse is a future optimization that must preserve the
`LocalModel` contract established by RFC-0007.

### 4.2 Immutable Loaded Configuration

The provider stores only the configuration fields it needs:

```rust
struct LoadedConfig {
    model_path: PathBuf,
    context: u32,
    threads: u32,
    gpu_layers: u32,
}
```

This avoids depending on the full `ResolvedConfig` type, which may
gain fields (timeouts, telemetry, provider selection) unrelated to
llama.cpp. The provider depends only on what it actually uses.

### 4.3 Model Info Population

`info()` returns a valid `ModelInfo` at all times:

- Before load: backend = "llama.cpp", name = "", version = "unknown"
- After load: backend = "llama.cpp", name from GGUF metadata,
  version from GGUF metadata

This keeps callers from having to reason about optional metadata
while allowing metadata enrichment after loading.

---

## 5. LlamaCppModel Struct

```rust
pub struct LlamaCppModel {
    model: Option<LlamaModel>,
    info: ModelInfo,
    loaded_config: Option<LoadedConfig>,
}
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `model` | `Option<LlamaModel>` | Loaded GGUF model. `None` before `load()`. |
| `info` | `ModelInfo` | Metadata. Updated after successful load. |
| `loaded_config` | `Option<LoadedConfig>` | Captured config subset. `None` before load. |

### Construction

```rust
impl LlamaCppModel {
    pub fn new() -> Self {
        Self {
            model: None,
            info: ModelInfo {
                name: "llama.cpp".into(),
                version: "unknown".into(),
                backend: "llama.cpp".into(),
            },
            loaded_config: None,
        }
    }
}
```

---

## 6. LocalModel Implementation

### load(config)

```text
load(config)
    ↓
validate model_path exists
    ↓
capture LoadedConfig from ResolvedConfig
    ↓
load GGUF model from disk
    ↓
extract metadata (name, version) from GGUF
    ↓
store model, config, and updated info
    ↓
return Ok(())
```

On failure: returns `ProviderError::Model`, model stays `None`.

### infer(request)

```text
infer(request)
    ↓
assert model is loaded (else return error)
    ↓
create inference session
    ↓
execute inference with prompt, temperature, max_tokens
    ↓
collect generated output
    ↓
destroy session
    ↓
return InferenceResponse { text: output }
```

### unload()

```text
unload()
    ↓
drop model (releases native resources)
    ↓
set model = None
    ↓
set loaded_config = None
    ↓
reset info to defaults
    ↓
return Ok(())
```

### info()

Always returns a valid `ModelInfo`. Before load, returns defaults.
After load, returns GGUF-extracted metadata.

---

## 7. Configuration Mapping

`ResolvedConfig` fields mapped to llama.cpp parameters:

| ResolvedConfig | llama.cpp | Notes |
|----------------|-----------|-------|
| `model_path` | model file path | GGUF file to load |
| `context` | context size | Token window |
| `threads` | thread count | CPU inference threads |
| `gpu_layers` | GPU offload layers | GPU memory allocation |

Fields not used by llama.cpp (e.g., `temperature`, `max_tokens`,
`model_name`) are passed through inference parameters, not engine
configuration.

---

## 8. Tests

### Unit Tests (no GGUF required)

| Test | Validates |
|------|-----------|
| `new_constructs_with_defaults` | Initial state is clean |
| `info_before_load_returns_defaults` | Pre-load metadata |
| `load_fails_nonexistent_path` | Missing GGUF returns error |
| `infer_before_load_returns_error` | No panic, returns error |
| `load_unload_load_unload_cycle` | No native resource leaks |
| `infer_after_unload_returns_error` | Unload invalidates inference |
| `llama_cpp_model_is_send_sync` | Thread safety |

### Integration Tests (feature-gated)

| Test | Validates |
|------|-----------|
| `load_populates_info_from_gguf` | GGUF metadata extraction |
| `infer_returns_generated_text` | End-to-end inference |
| `load_unload_cleans_up_resources` | GPU memory released |

Integration tests require a GGUF model file and are gated behind
`#[cfg(feature = "integration")]`.

### Lifecycle Tests (via LocalProvider)

| Test | Validates |
|------|-----------|
| `provider_starts_uninitialized` | State machine initial state |
| `load_transitions_to_ready` | State: Uninitialized → Loading → Ready |
| `unload_transitions_to_uninitialized` | State: Ready → Unloading → Uninitialized |
| `failed_load_transitions_to_failed` | State: Loading → Failed |
| `failed_can_retry_to_loading` | State: Failed → Loading → Ready |
| `health_reflects_state` | ProviderHealth mapping |

---

## 9. Invariants

Every conforming implementation MUST satisfy:

1. `LlamaCppModel` implements `LocalModel` and is `Send + Sync`.
2. Inference sessions are created and destroyed per call.
3. No hidden mutable state persists between inference calls.
4. `unload()` drops the native model and releases all resources.
5. `info()` always returns a valid `ModelInfo`.
6. `load()` captures only the config fields it needs.
7. No changes to contract, runtime, kernel, or agent.
8. The provider is a single crate at `providers/llama-cpp/`.

---

## 10. Non-Goals

The following are explicitly out of scope for this RFC:

- Streaming token generation
- Conversation memory or chat history
- Tool calling inside the provider
- Automatic model download or distribution
- Session reuse or persistent context optimization
- GPU memory pooling
- Batch inference
- Multiple model support
- Quantization selection
- Benchmarking or performance optimization

---

## 11. Exit Criteria

RFC-0008 is complete when:

- `LlamaCppModel` implements `LocalModel` with stateless inference.
- Unit tests pass without requiring a GGUF model file.
- Integration tests (feature-gated) pass with a GGUF model.
- `LocalProvider<LlamaCppModel>` lifecycle tests pass.
- No changes exist in contract, runtime, kernel, execution, or agent.
- `cargo fmt`, `cargo clippy -D warnings`, and `cargo test --workspace`
  all pass.
- RFC-0008 is committed and RFC index is updated.

---

**End of RFC-0008**
