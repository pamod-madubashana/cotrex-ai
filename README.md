<p align="center">
  <img src="assets/cotrex.png" alt="Cotrex" width="220">
</p>

<p align="center">
  <strong>Protocol-first AI runtime for the Cotrex kernel</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/built_with-Rust-orange.svg" alt="Built with Rust">
  <img src="https://img.shields.io/badge/version-0.8.0-blue.svg" alt="Version 0.8.0">
  <img src="https://img.shields.io/badge/edition-2024-purple.svg" alt="Rust 2024">
  <img src="https://img.shields.io/badge/license-MIT-green.svg" alt="MIT License">
</p>

<p align="center">
  <a href="#what-is-cotrex-ai">About</a> &bull;
  <a href="#architecture">Architecture</a> &bull;
  <a href="#quick-start">Quick Start</a> &bull;
  <a href="#documentation">Docs</a>
</p>

---

## What is Cotrex AI

Cotrex AI Runtime (`cotrex-ai`) is the AI execution layer for the Cotrex agent OS. It abstracts inference providers behind a stable, typed protocol and provides an orchestration pipeline that turns capability requests into model inference.

- **What it does**: Provides AI inference and orchestration to the Cotrex kernel
- **Why it's useful**: Models are replaceable, the protocol is not
- **How it works**: Takes capability requests, builds context, assembles prompts, dispatches to providers, parses responses

---

## Architecture

```text
Agent
    │
    ▼
MCP (JSON-RPC)
    │
    ▼
Orchestrator
    │
    ├── ContextSource (read-only, returns InferenceContext)
    │       └── KernelContextSource (bridges kernel projections)
    │
    ├── PromptAssembler (combines context + capability data)
    ├── CapabilityProvider (executes inference)
    ├── OutputParser (classifies model output)
    └── CapabilityResponseParser (extracts typed response)
    │
    ▼
Provider
    │
    ▼
llama.cpp / mock / json-fixture
```

**Kernel** — Owns project state, event sourcing, observation. Standalone crate with no internal dependencies.

**Runtime** — Orchestration pipeline, provider abstraction, context source trait. Depends only on contract.

**Providers** — Implement `CapabilityProvider` trait, execute AI inference. Optional (behind `local-model` feature).

**Composition root** (Cotrex binary) — Wires kernel, runtime, and providers together. The bridge between kernel projections and runtime context lives here.

---

## Workspace

```text
cotrex-ai/
├── contract/        # Protocol types (no logic, no providers)
├── runtime/         # Orchestration pipeline, ContextSource trait, provider abstraction
├── kernel/          # Event Store, projections, observation pipeline
├── execution/       # Execution engine, registry, and built-in executors
├── providers/
│   ├── llama-cpp/   # llama.cpp FFI provider (optional)
│   ├── mock/        # Deterministic mock responses
│   └── json/        # JSON fixture provider
├── examples/        # Usage examples
├── fixtures/        # JSON response fixtures
├── RFC/             # Protocol definitions and implementation strategy
└── ADR/             # Architectural Decision Records
```

---

## Quick Start

### Prerequisites

- Rust 2024 edition
- CMake (for llama.cpp compilation)

### Build

```bash
cargo build --workspace
```

### Build with local inference

```bash
cargo build --workspace --features local-model
```

### Test

```bash
cargo test --workspace
```

### Lint

```bash
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

---

## Key Types

### Protocol (contract)

| Type | Purpose |
|------|---------|
| `ProtocolVersion` | Exact version match required |
| `CapabilityRequest` | Request enum (`BuildSummary`, `ExplainRust`) |
| `CapabilityResponse` | Response enum |
| `RequestMetadata` | UUID + timestamp, attached to every request |
| `ProviderInfo` | Provider metadata (name, version, capabilities) |
| `ProviderHealth` | Provider health status |

### Runtime

| Type | Purpose |
|------|---------|
| `CapabilityProvider` | Core trait: `Send + Sync`, `info()`, `health()`, `execute()` |
| `ContextSource` | Read-only trait: `context() -> Result<InferenceContext, RuntimeError>` |
| `Orchestrator` | Pipeline: context → prompt → provider → parse → normalize |
| `InferenceContext` | Workspace state for AI: recent changes, status, file count |
| `NullContextSource` | Fallback when no live workspace is available |

### Kernel

| Type | Purpose |
|------|---------|
| `Event` | Envelope: `id`, `sequence`, `occurred_at`, `payload` |
| `EventPayload` | Enum with `FileChanged` variant |
| `EventStore` | Trait-based append-only store with sequence ordering |
| `MemoryEventStore` | In-memory implementation for tests |
| `PersistentEventStore` | File-backed JSONL implementation for production |
| `ProjectionEngine` | Multi-projection coordinator with failure isolation |
| `AiContextProjection` | Semantic summary for AI: workspace status, recent changes |
| `ObservationPipeline` | Filter → Translate → Append pipeline |

### Execution Runtime

| Type | Purpose |
|------|---------|
| `ExecutionEngine` | Orchestrates execution lifecycle and event recording |
| `ExecutorRegistry` | Registers and resolves execution capabilities |

---

## Milestones

| Milestone | Description | Status |
|-----------|-------------|--------|
| A | Foundation — contract, runtime, mock provider | ✅ Complete |
| B | Contracts — protocol types, capability dispatch | ✅ Complete |
| C | Providers — mock, json-fixture, extension methods | ✅ Complete |
| D | Kernel — event store, projections, observation | ✅ Complete |
| E | Real Inference — llama.cpp provider activation | ✅ Complete |
| F | Model Manager — download, resolve, registry | ✅ Complete |
| G | Provider Abstraction — lazy loading, factory trait | ✅ Complete |
| H | Intelligence Orchestration — orchestrator pipeline, MCP migration | ✅ Complete |
| I | Workspace Intelligence — ContextSource, kernel bridge | ✅ Complete |

---

## Documentation

| Document | Purpose |
|----------|---------|
| [Vision.md](Vision.md) | Why Cotrex exists |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Canonical source of truth |
| [AGENTS.md](AGENTS.md) | Agent instructions |
| [RFC/](RFC/) | Protocol definitions and implementation strategy |
| [ADR/](ADR/) | Architectural Decision Records |

---

## RFCs

| RFC | Title | Status |
|-----|-------|--------|
| [RFC-0001](RFC/RFC-0001-kernel-event-store.md) | Kernel Event Store | Implemented |
| [RFC-0002](RFC/RFC-0002-projection-engine.md) | Projection Engine | Implemented |
| [RFC-0003](RFC/RFC-0003-observation-pipeline.md) | Observation Pipeline | Implemented |
| [RFC-0004](RFC/RFC-0004-execution-engine.md) | Execution Engine | Implemented |
| [RFC-0005](RFC/RFC-0005-ai-runtime-integration.md) | AI Runtime Integration | Implemented |
| [RFC-0006](RFC/RFC-0006-persistent-event-store.md) | Persistent Event Store | Implemented |
| [RFC-0007](RFC/RFC-0007-local-provider-runtime.md) | Local Provider Runtime | Implemented |
| [RFC-0008](RFC/RFC-0008-llama-cpp-provider.md) | llama.cpp Provider | Implemented |

---

## ADRs

| ADR | Title | Status |
|-----|-------|--------|
| ADR-0001 | Event Sourcing | Accepted |
| [ADR-0002](ADR/ADR-0002-protocol-versioning.md) | Protocol Versioning Strategy | Accepted |
| ADR-0003 | Closed Capability Protocol | Accepted |
| ADR-0004 | Cargo Workspace | Accepted |
| ADR-0005 | AI as Advisory Layer | Accepted |
| [ADR-0006](ADR/ADR-0006-event-store-persistence-strategy.md) | Event Store Persistence Strategy | Accepted |

---

## License

[MIT](LICENSE)
