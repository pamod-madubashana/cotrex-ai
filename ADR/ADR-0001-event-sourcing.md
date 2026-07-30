# ADR-0001: Event Sourcing

**Status:** Accepted

---

## Context

The kernel requires a state management strategy that provides a complete audit history, deterministic state reconstruction, and the ability to rebuild derived state from raw facts.

Modern development workflows produce complex state transitions. Build results, file changes, execution outcomes, and AI interactions all contribute to project reality. Without a unified state strategy, this reality becomes fragmented across mutable caches and implicit assumptions.

The kernel must answer: what happened, when did it happen, and what is the current state as a consequence?

---

## Decision

**Event sourcing is the kernel's state management strategy.**

All state changes are recorded as immutable events in an append-only log. The current state is derived by replaying events from the beginning of history. No event is ever mutated or deleted.

Projections derive queryable read models from the event stream. Projections are rebuilt from events, never mutated directly. Multiple projections may exist for different query patterns.

State reconstruction is deterministic: given the same sequence of events, the same state is always produced.

---

## Alternatives Considered

### Mutable CRUD State

Store current state directly in mutable records. Update in place when state changes.

**Rejected because:**
- No replay capability — state changes are lost after mutation
- No audit history — previous states are overwritten
- State reconstruction is impossible without external logging
- Implicit mutable state makes debugging non-deterministic
- Violates the principle that no implicit state exists in the system

---

## Consequences

### Positive

- **Auditability:** Every state change is recorded as an immutable event
- **Replayability:** State can be reconstructed from any point in history
- **Determinism:** Same events always produce same state
- **Debugging:** Full history available for post-mortem analysis
- **Projections:** Derived read models can be rebuilt without data loss

### Negative

- **Storage growth:** Event log grows without bound (requires eventual compaction or archival)
- **Complexity:** State reconstruction requires replay logic
- **Write amplification:** Every state change produces an event, not a direct mutation
- **Eventual consistency:** Projections may lag behind event writes

---

## References

- ARCHITECTURE.md: Event Sourcing philosophy (Architectural Invariants)
- RFC-0001: Kernel Event Store
- Vision.md: Event-sourced history goal
- kernel/src/store.rs: EventStore trait, MemoryEventStore
