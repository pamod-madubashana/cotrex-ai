# Architecture

This is the canonical architecture document for Cotrex. All other documents derive from this one.

---

## Documentation Hierarchy

| Document | Answers | Contains |
|----------|---------|----------|
| `Vision.md` | Why does Cotrex exist? | Philosophy, long-term goals, guiding principles. No implementation. |
| `ARCHITECTURE.md` | What are the major subsystems? | System boundaries, component responsibilities, dependency direction, diagrams. No implementation details. |
| `RFC/` | How is a subsystem implemented? | Protocol definitions, APIs, implementation strategy. |
| `ADR/` | Why was a technology or design chosen? | Alternatives considered, decision, consequences. |

---

## System Layers

Cotrex is organized into four architectural layers. Each layer depends only on the layers above it. No layer depends on layers below it.

```text
                 Cotrex

            Kernel Layer
                  │
                  ▼
        Intelligence Brain
                  │
                  ▼
          cotrex-ai Runtime
                  │
                  ▼
      Capability Provider API
                  │
      ┌───────────┼───────────┐
      │           │           │
  llama.cpp     Candle      ONNX
```

### Layer 1: Kernel

The kernel owns project state. It is the single source of truth for all project reality.

Responsibilities:

- Event sourcing and state management
- File system observation
- Event bus and projections
- Execution orchestration
- Capability dispatch to the Intelligence Brain

The kernel never bypasses the Intelligence Brain. The kernel only produces structured capability requests.

### Layer 2: Intelligence Brain

The Intelligence Brain is the AI orchestration layer. It owns the decision of when and how to invoke AI capabilities.

Responsibilities:

- Orchestration of AI workflows
- Deciding when to invoke capabilities
- Interpreting AI responses
- Coordinating with the kernel

The Intelligence Brain delegates all AI execution to the cotrex-ai workspace. The Intelligence Brain owns orchestration. cotrex-ai owns capability execution. Inference providers remain implementation details beneath the runtime.

### Layer 3: cotrex-ai Runtime

cotrex-ai is the implementation of the Intelligence Brain's AI Runtime. It is NOT a competing architecture. It is the execution layer that the Intelligence Brain delegates to.

Responsibilities:

- Provider abstraction
- Capability dispatch
- Runtime lifecycle
- Error handling
- Validation

The runtime knows nothing about specific models. The protocol is the product.

### Layer 4: Inference Providers

Inference providers sit beneath cotrex-ai. They implement the CapabilityProvider trait and execute actual AI inference.

Examples: llama.cpp, Candle, ONNX, remote APIs.

Providers are implementation details. They can be swapped without changing the runtime or the kernel.

---

## Subsystems

### Kernel Subsystems

#### Event Store

The event store records all state changes as immutable events. The current state is derived by replaying events.

#### File System Observation

Watches the project file system for changes and emits FileChanged events to the event bus.

#### Event Bus

Routes events from producers (observation, execution) to consumers (projections, Intelligence Brain).

#### Projections

Derive queryable state from the event stream. Projections are rebuilt from events, never mutated directly.

#### Execution

Executes commands and manages process lifecycle. Writes execution results back to the event store.

### Intelligence Brain

Orchestrates AI capabilities. Owns the decision logic for when to invoke which capability. Delegates execution to cotrex-ai.

### cotrex-ai Runtime

See [Layer 3: cotrex-ai Runtime](#layer-3-cotrex-ai-runtime) above.

---

## Dependency Direction

The dependency graph is strictly hierarchical:

```text
Kernel
  ├── depends on → Protocol (contract crate)
  └── depends on → Intelligence Brain

Intelligence Brain
  └── depends on → Protocol (contract crate)

cotrex-ai Runtime
  └── depends on → Protocol (contract crate)

Providers
  └── depend on → Runtime + Protocol
```

No layer depends on layers below it. The kernel never imports provider code. Providers never import kernel code.

---

## Protocol Ownership

The protocol is owned by the kernel. It is defined in the `contract` crate.

- Capability definitions are owned by the Kernel.
- Providers cannot introduce new capabilities.
- Protocol changes require version changes.
- The protocol is intentionally closed.

Adding a capability is a protocol revision. It requires incrementing the protocol version and updating all implementations.

---

## Concurrency

The runtime may invoke providers concurrently. Multiple capability requests can execute in parallel.

Provider implementations must explicitly document whether they support concurrent execution. Thread-safety requirements will be finalized during the first real provider implementation.

Do not introduce async APIs yet. Do not redesign the CapabilityProvider trait. This is documentation only.

---

## Deferred Architectural Decisions

The following are intentionally out of scope until validated by implementation:

- Provider routing
- Provider registry
- Scheduling
- Streaming responses
- Distributed inference
- Plugin ecosystem
- Model lifecycle management
- Provider selection strategies

These will only be introduced when justified by production usage.

---

## Implementation Milestones

### Milestone 1: Completed

- cotrex-ai protocol (contract crate)
- Runtime crate (CapabilityProvider trait)
- Mock provider
- JSON fixture provider
- 30 tests passing

### Milestone 2: Completed

- Documentation consolidation
- Architecture cleanup
- AGENTS.md
- ADR-0002: Protocol Versioning Strategy

### Milestone 3: RFC-0001

Minimal event-sourced kernel.

Scope:

- FileChanged event
- Event Store
- One projection
- Replay
- Backpressure and event ordering validation

No git. No builds. No AI.

### Milestone 4: Real AI Provider

First real inference provider (Candle or llama.cpp).

### Milestone 5: Remaining Kernel Capabilities

Remaining kernel subsystems: execution, projections, Intelligence Brain orchestration.

---

## Kernel MVP Scope

> **Kernel MVP Scope Lock**
>
> The objective is to validate the event model only.

### Allowed

- FileChanged event
- Event Store
- Replay
- One Projection
- Ordering validation
- Backpressure validation

### Explicitly Excluded

- Git
- Build system
- AI
- Execution
- Search
- Knowledge Graph
- Plugins
- Networking

---

## Architectural Invariants

The following are frozen and must not be modified:

- Event sourcing philosophy
- CQRS
- Kernel ownership
- Closed capability protocol
- CapabilityProvider architecture
- Protocol versioning
- Cargo workspace layout

---

## Workspace Layout

```text
cotrex-ai/
├── contract/        # Protocol types only (no logic, no providers)
├── runtime/         # CapabilityProvider trait + extension methods
├── providers/
│   ├── mock/        # Deterministic mock responses
│   └── json/        # JSON fixture provider
├── examples/        # Usage examples
├── fixtures/        # JSON response fixtures
├── RFC/             # Protocol definitions and implementation strategy
└── ADR/             # Architectural Decision Records
```
