# RFC-0005: AI Runtime Integration

**Status:** Implemented

---

## 1. Purpose

The cotrex-ai runtime provides the execution layer for AI capabilities.
It owns the abstraction between capability requests and provider
execution, handling protocol translation, error propagation, and
provider lifecycle.

The runtime exists so the kernel never talks to inference engines
directly. The kernel produces structured capability requests. The
runtime executes them against replaceable providers. The kernel
receives structured responses.

This boundary keeps the kernel deterministic. The runtime is
replaceable. Providers are implementation details.

---

## 2. Architecture Position

The runtime sits between the Intelligence Brain and inference
providers.

```text
             Kernel
               │
               ▼
       Intelligence Brain
               │
               ▼
         cotrex-ai Runtime
               │
               ▼
      CapabilityProvider
               │
       ┌───────┼───────┐
       ▼       ▼       ▼
   llama.cpp  Mock   Future
```

The runtime depends only on the contract crate. It never imports
kernel code. Providers depend on the runtime. No layer depends
on layers below it.

---

## 3. Runtime Responsibilities

The runtime owns:

- **Provider abstraction** — the `CapabilityProvider` trait that
  every backend implements.
- **Protocol translation** — converting `CapabilityRequest` into
  typed inference calls and `CapabilityResponse` back.
- **Error propagation** — translating provider failures into
  typed runtime errors.
- **Provider lifecycle** — managing health, metadata, and state
  transitions.
- **Extension methods** — ergonomic helpers that delegate to the
  core trait.

The runtime does NOT own:

- Prompt construction (Intelligence Brain)
- Event storage (Kernel)
- Model loading or inference (Provider)
- Context assembly (RFC-0009)

---

## 4. Provider Abstraction

Every backend implements `CapabilityProvider`. The trait is
intentionally minimal: metadata, health, and execution.

**Why minimal:**

- Providers are `Send + Sync` — the runtime may share them
  across threads.
- Prompt building is private to each provider — different models
  have different prompting strategies.
- The API is synchronous — inference is CPU-bound; async would
  add complexity without benefit.

**What providers report:**

- `info()` — metadata (name, version, supported capabilities)
- `health()` — current health status
- `execute()` — run a capability request

**What providers never do:**

- Construct prompts from raw data
- Access the Event Store directly
- Know which model is being used (that is internal)

The provider boundary is the most important contract in the
system. Everything above it is typed. Everything below it is
replaceable.

---

## 5. Runtime Interaction Model

Capability execution follows a typed flow:

```text
CapabilityRequest
       │
       ▼
CapabilityProvider::execute()
       │
       ▼
CapabilityResponse
```

The runtime enforces:

- **Type safety** — requests and responses are enums, not strings
- **Capability matching** — providers report supported capabilities;
  unsupported requests fail fast
- **No string parsing** — all data flows as typed structures

This is why the protocol is closed (ADR-0003). Providers cannot
invent new capability types. Adding a capability is a protocol
revision.

---

## 6. Runtime Error Model

Errors are split into two layers:

**Contract errors** — protocol-level failures:

- Invalid request
- Unsupported protocol version

**Runtime errors** — execution-level failures:

- Provider failure (backend crashed, unavailable)
- Invalid response (wrong type returned)
- Capability error (wraps contract errors)

This split keeps protocol concerns separate from execution
concerns. The kernel never sees provider internals. The runtime
translates failures into typed errors.

---

## 7. Runtime Invariants

1. The `CapabilityProvider` trait is unchanged by provider
   implementations.
2. Providers remain replaceable without runtime changes.
3. All capability requests and responses are typed enums.
4. The runtime never constructs prompts.
5. The runtime never accesses the Event Store directly.
6. Backend details never leak above the provider boundary.
7. Error propagation preserves type information.

---

## 8. Non-Goals

- Model selection or routing
- Prompt construction
- Context assembly
- Event storage
- Conversation memory
- Streaming responses
- Distributed inference
- Plugin ecosystem
- Model lifecycle management

---

## 9. Exit Criteria

RFC-0005 is complete when:

- `CapabilityProvider` trait is defined and documented.
- Runtime error types are defined with clear separation.
- Providers remain replaceable without kernel changes.
- Mock implementations compile against the trait.
- No backend-specific code exists in the runtime.
- `cargo fmt`, `cargo clippy -D warnings`, `cargo test` all pass.

---

**End of RFC-0005**
