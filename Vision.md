# Vision

Why does Cotrex exist?

---

## Philosophy

Cotrex is a system kernel for AI-augmented software engineering.

It exists because modern development workflows are fragmented. Editors, build systems, version control, and AI assistants operate as isolated tools with no shared understanding of project state. Cotrex unifies these concerns under a single kernel that owns project reality and exposes structured capabilities to AI runtimes.

The kernel is the source of truth. AI is a consumer of that truth.

---

## Long-Term Goals

1. **Unified project state** — The kernel maintains a single, queryable representation of project reality. Every subsystem reads from and writes to this shared state.

2. **Structured AI integration** — AI capabilities are invoked through typed protocols, not free-form prompts. The kernel never surrenders control to an AI model.

3. **Provider independence** — AI inference backends are replaceable without changing the kernel. The protocol is stable; the implementation is not.

4. **Event-sourced history** — All state changes are recorded as events. The current state can be replayed from history. No implicit state.

5. **Composable capabilities** — The system exposes a closed set of capabilities. Adding a new capability is a protocol revision, not a plugin.

---

## Guiding Principles

### Kernel Owns Reality

The kernel owns project state. The runtime never bypasses the kernel. The runtime only receives structured requests produced by the kernel.

### AI Is A Consumer

AI is not the center of the architecture. It consumes structured capability requests and produces structured responses. Replacing the model must never require changing the kernel.

### Contracts Over Models

Models are implementation details. The protocol is the product. Every provider implements the same protocol regardless of whether it uses a local model, a remote API, or a mock.

### Structured I/O Only

No free-form prompting exists between the kernel and the runtime. Every interaction is represented as typed data.

### Closed Capability System

Capabilities are closed. Providers cannot invent new capability types. Adding a capability is a protocol revision.

### Implementation Follows Architecture

Architecture decisions are made before code is written. Documentation precedes implementation. The architecture is the source of truth; code is its expression.
