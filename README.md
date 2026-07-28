# Cotrex AI Runtime

A protocol-first AI runtime for the Cotrex kernel.

---

## What Is This?

Cotrex AI Runtime (`cotrex-ai`) is the implementation of the Intelligence Brain's AI execution layer. It abstracts AI inference providers behind a stable, typed protocol while exposing a deterministic interface to the Cotrex kernel.

The protocol is the product. Models are implementation details.

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
                  ▼
      Capability Provider API
                  │
      ┌───────────┼───────────┐
      │           │           │
  llama.cpp     Candle      ONNX
```

**Layer 1: Kernel** — Owns project state, event sourcing, observation.

**Layer 2: Intelligence Brain** — Orchestrates AI workflows, decides when to invoke capabilities.

**Layer 3: cotrex-ai Runtime** — Provider abstraction, capability dispatch, error handling.

**Layer 4: Inference Providers** — Implement `CapabilityProvider` trait, execute AI inference.

---

## Workspace

```text
cotrex-ai/
├── contract/        # Protocol types (no logic, no providers)
├── runtime/         # CapabilityProvider trait + extension methods
├── kernel/          # Event Store, projections, event model
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
- [RTK](https://github.com/rtk-ai/rtk) (recommended for running commands)

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
| `EventStore` | Append-only store with sequence ordering |
| `FileChangeProjection` | Derives file state from events |

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

## Milestones

| Milestone | Description | Status |
|-----------|-------------|--------|
| 1 | Protocol + Runtime + Mock provider | ✅ Complete |
| 2 | Documentation consolidation | ✅ Complete |
| 3 | Documentation frozen | ✅ Complete |
| 4 | RFC-0001: Kernel Event Store | ✅ Complete (in-memory) |
| 5 | RFC-0002: Projection Engine | ✅ Complete |
| 6 | RFC-0003: Observation Pipeline | ⏳ Pending |
| 7 | RFC-0004: Execution Engine | ⏳ Pending |
| 8 | Real AI provider | ⏳ Pending |
| 9 | RFC-0005: AI Runtime Integration | ⏳ Pending |

---

## RFCs

| RFC | Title | Status |
|-----|-------|--------|
| [RFC-0001](RFC/RFC-0001-kernel-event-store.md) | Kernel Event Store | Implemented |
| [RFC-0002](RFC/RFC-0002-projection-engine.md) | Projection Engine | Implemented |
| [RFC-0003](RFC/RFC-0003-observation-pipeline.md) | Observation Pipeline | Draft |
| RFC-0004 | Execution Engine | Planned |
| RFC-0005 | AI Runtime Integration | Planned |

---

## ADRs

| ADR | Title | Status |
|-----|-------|--------|
| ADR-0001 | Event Sourcing | Planned |
| [ADR-0002](ADR/ADR-0002-protocol-versioning.md) | Protocol Versioning Strategy | Accepted |
| ADR-0003 | Closed Capability Protocol | Planned |
| ADR-0004 | Cargo Workspace | Planned |
| ADR-0005 | AI as Advisory Layer | Planned |
| [ADR-0006](ADR/ADR-0006-event-store-persistence-strategy.md) | Event Store Persistence Strategy | Proposed |

---

## License

[MIT](LICENSE)
