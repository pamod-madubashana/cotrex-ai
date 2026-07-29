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

Cotrex AI Runtime (`cotrex-ai`) is the implementation of the Intelligence Brain's AI execution layer. It abstracts AI inference providers behind a stable, typed protocol while exposing a deterministic interface to the Cotrex kernel.

- **What it does**: Provides AI inference capabilities to the Cotrex kernel
- **Why it's useful**: Models are replaceable, the protocol is not
- **How it works**: Takes capability requests, dispatches to providers, returns typed responses

---

## Architecture

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
        ┌─────────┴─────────┐
        ▼                   ▼
 Capability Provider     Execution Runtime
        │                   │
        ▼                   ▼
 Inference Providers    Executor Registry
                            │
              ┌─────────────┼─────────────┐
              ▼             ▼             ▼
          Command       FileWrite      FileDelete
```

**Layer 1: Kernel** — Owns project state, event sourcing, observation.

**Layer 2: Intelligence Brain** — Orchestrates AI workflows, decides when to invoke capabilities.

**Layer 3: cotrex-ai Runtime** — Provider abstraction, capability dispatch, execution orchestration, and runtime error handling.

**Layer 4: Inference Providers** — Implement `CapabilityProvider` trait, execute AI inference.

---

## Workspace

```text
cotrex-ai/
├── contract/        # Protocol types (no logic, no providers)
├── runtime/         # CapabilityProvider trait + extension methods
├── kernel/          # Event Store, projections, event model
├── execution/       # Execution engine, registry, and built-in executors
├── providers/
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

### Build

```bash
cargo build --workspace
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
| `CapabilityProviderExt` | Ergonomic methods: `.build_summary()`, `.explain_rust()` |
| `RuntimeError` | Execution errors: `Provider`, `InvalidResponse`, `Capability` |

### Kernel

| Type | Purpose |
|------|---------|
| `Event` | Envelope: `id`, `sequence`, `occurred_at`, `payload` |
| `EventPayload` | Enum with `FileChanged` variant |
| `EventStore` | Trait-based append-only store with sequence ordering |
| `MemoryEventStore` | In-memory implementation for tests and lightweight usage |
| `PersistentEventStore` | File-backed JSONL implementation for production |
| `FileChangeProjection` | Derives file state from events |

### Execution Runtime

| Type | Purpose |
|------|---------|
| `ExecutionEngine` | Orchestrates execution lifecycle and event recording |
| `ExecutorRegistry` | Registers and resolves execution capabilities |
| `CommandExecutor` | Executes OS commands with controlled output handling |
| `FileWriteExecutor` | Writes files within a validated working directory |
| `FileDeleteExecutor` | Deletes files within a validated working directory |
| `ExecutionResult` | Transient execution output, including stdout/stderr |

---

## Milestones

| Milestone | Description | Status |
|-----------|-------------|--------|
| 1 | Protocol + Runtime + Mock provider | ✅ Complete |
| 2 | Documentation consolidation | ✅ Complete |
| 3 | Documentation frozen | ✅ Complete |
| 4 | RFC-0001: Kernel Event Store | ✅ Complete |
| 5 | RFC-0002: Projection Engine | ✅ Complete |
| 6 | RFC-0003: Observation Pipeline | ✅ Complete |
| 7 | RFC-0004: Execution Engine | ✅ Complete |
| 8 | Agent Reasoning Layer | ✅ Complete |
| 9 | RFC-0005: AI Runtime Integration | ✅ Complete |
| 10 | RFC-0006: Persistent Event Store | ✅ Complete |

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
| [RFC-0007](RFC/RFC-0007-local-provider-runtime.md) | Local Provider Runtime | Accepted |

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
