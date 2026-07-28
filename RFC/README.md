# RFCs

Request for Comments — protocol definitions and implementation strategy.

## Purpose

RFCs answer: **How is a subsystem implemented?**

They contain:

- Protocol definitions
- API specifications
- Implementation strategy
- Trade-offs and rationale

## Naming Convention

```
RFC-NNNN-title.md
```

Example: `RFC-0001-kernel-event-store.md`

## Process

1. Author creates RFC with status `Draft`
2. Review and discussion
3. Status changes to `Accepted` or `Rejected`
4. Implementation begins only after acceptance

## Status Labels

- `Draft` — Under development
- `Accepted` — Approved for implementation
- `Implemented` — Implementation complete
- `Rejected` — Not proceeding
- `Superseded` — Replaced by a later RFC

---

## RFC Index

| RFC | Title | Status |
|-----|-------|--------|
| RFC-0001 | Kernel Event Store | Implemented |
| RFC-0002 | Projection Engine | Draft |
| RFC-0003 | Observation Pipeline | Planned |
| RFC-0004 | Execution Engine | Planned |
| RFC-0005 | AI Runtime Integration | Planned |

---

## RFC-0001 Preparation Notes

RFC-0001 must define the following. These are implementation requirements, not design suggestions.

### Mandatory Definitions

#### Event Ordering Guarantees

- What ordering guarantees does the event store provide?
- Are events totally ordered, partially ordered, or unordered?
- What happens if events arrive out of order?

#### Event Store Write Ordering

- How are concurrent writes handled?
- Is there a write lock, optimistic concurrency, or append-only semantics?
- What is the write throughput requirement?

#### Replay Guarantees

- Can the event store be replayed from an arbitrary point?
- Is replay deterministic?
- What is the replay performance characteristics?

#### Projection Consistency

- Are projections eventually consistent or strongly consistent?
- How are projection updates ordered relative to event writes?
- What happens if a projection fails mid-update?

#### Backpressure Behavior

- What happens when the event store is under load?
- Is there backpressure from producers to consumers?
- How are slow consumers handled?

### Implementation Scope

The Kernel MVP (Milestone 3) is locked to:

- FileChanged event
- Event Store
- One projection
- Replay
- Ordering validation
- Backpressure validation

No git. No builds. No AI. No execution. No search. No knowledge graph. No plugins. No networking.

---

## References

- ARCHITECTURE.md: Kernel Subsystems section
- ADR-0002: Protocol Versioning Strategy
- Vision.md: Event-sourced history goal
