# RFC-0002: Projection Engine

**Status:** Implemented
**Milestone:** 5
**Depends on:** RFC-0001 (Kernel Event Store)

---

## 1. Purpose

This RFC defines the Projection Engine: the subsystem responsible for
transforming committed Event Store events into derived state.

Projections exist because:

- Raw event history is not suitable for direct consumption by most
  consumers.
- Consumers need semantic state, not sequential event logs.
- Different consumers require different views of the same history.
- Derived state is disposable and rebuildable; event history is not.

The Event Store remains the source of truth. Projections are
interpretations of that truth, not alternatives to it.

---

## 2. Glossary

- **Projection**: a component that consumes committed events and
  maintains derived state.
- **Derived State**: the output of a projection; rebuildable from event
  history alone.
- **Projection Instance**: a single, named projection with its own
  isolated state.
- **Projection Offset**: the sequence number of the last event processed
  by a projection.
- **Checkpoint**: a recorded projection offset, used to resume processing
  after interruption.
- **Rebuild**: the process of discarding derived state and recreating it
  from event replay.
- **Consumer Contract**: the public interface through which external
  systems read projection state.

---

## 3. Projection Model

```text
Event Store
      |
      v
Projection Engine
      |
      v
Derived State
```

Rules:

- Projections consume committed events.
- Projections do not create events.
- Projections do not modify the Event Store.
- Projections read from the Event Store; they never write to it.

---

## 4. Event Processing Guarantees

Projections process events in sequence order.

Requirements:

- Events are processed in sequence order.
- The sequence number is authoritative for ordering.
- Timestamps never determine processing order.
- Committed events are processed exactly according to Event Store replay
  semantics (RFC-0001, Section 5).
- No event is processed more than once within a single processing cycle.

---

## 5. Projection Lifecycle

A projection exists in one of the following states:

```text
Created → Initialized → Processing → [Failed | Rebuilding]
```

### State Definitions

| State | Description |
|-------|-------------|
| Created | Projection instance exists but has not loaded state. |
| Initialized | Projection has loaded or rebuilt state; ready to process. |
| Processing | Projection is actively consuming events. |
| Failed | Projection encountered an error; state may be invalid. |
| Rebuilding | Projection is discarding state and replaying from sequence 0. |

### Allowed Transitions

| From | To | Trigger |
|------|----|---------|
| Created | Initialized | First rebuild or checkpoint load |
| Initialized | Processing | Events available |
| Processing | Failed | Error during event processing |
| Failed | Rebuilding | Manual or automatic recovery |
| Rebuilding | Initialized | Rebuild complete |
| Processing | Rebuilding | Explicit rebuild request |

### Invalid Transitions

- Created → Processing (must initialize first)
- Failed → Processing (must rebuild first)
- Rebuilding → Processing (must complete rebuild first)

### Recovery Behavior

When a projection enters the Failed state:

1. The projection must not process new events.
2. The projection must not corrupt existing derived state.
3. Recovery requires an explicit Rebuilding transition.
4. Rebuilding replays from sequence 0, producing deterministic state.

---

## 6. Rebuild Semantics

A projection MUST be rebuildable from Event Store replay.

### Rebuild Contract

1. Discard all existing derived state.
2. Replay events from sequence 0.
3. Process events in sequence order.
4. Produce deterministic state.

### Example

Given Event Store with events:

```text
1: FileCreated(src/main.rs)
2: FileModified(src/main.rs)
3: FileCreated(Cargo.toml)
4: FileModified(src/main.rs)
```

Rebuild produces:

```text
src/main.rs  — Modified, 2 changes
Cargo.toml   — Created, 1 change
```

### Determinism

Rebuilding from the same Event Store state MUST produce identical derived
state, regardless of:

- when the rebuild occurs
- what state existed before the rebuild
- how many times the rebuild has been performed

---

## 7. Multiple Projection Support

The Projection Engine supports multiple concurrent projections.

```text
              Event Store
                    |
                    v
            Projection Engine
           /        |        \
   File Index   State View   AI Context
```

Rules:

- Multiple projections may exist simultaneously.
- Projections are isolated from each other.
- One projection failure cannot corrupt other projections.
- Projection state is independent; projections do not share derived
  state.
- Each projection maintains its own offset and checkpoint.

---

## 8. Projection State Contract

Projection output is a public consumer contract.

### Requirements

Projection output MUST:

- Expose semantic state relevant to consumers.
- Hide storage implementation details.
- Remain stable across rebuilds (same input → same output structure).
- Be self-contained; consumers should not need to consult the Event
  Store directly.

Projection output MUST NOT expose:

- Event Store sequence numbers.
- Internal event IDs.
- Storage offsets.
- Backend implementation details.

unless explicitly required by a future consumer contract.

---

## 9. AI Context Projection

The AI Context Projection is a special projection that summarizes
system state for AI consumption.

### Requirements

AI Context Projection MUST:

- Summarize relevant system state.
- Reduce unnecessary information.
- Provide semantic understanding suitable for AI providers.
- Remain deterministic across rebuilds.

AI Context Projection MUST NOT:

- Become an Event Store dump.
- Expose kernel internals.
- Require AI providers to understand event history.
- Contain raw event data.

### Example

Bad (exposes internals):

```json
{
  "sequence": 500,
  "event_id": "uuid",
  "operation": "Modified"
}
```

Good (semantic state):

```json
{
  "workspace_status": "active",
  "recent_changes": [
    "src/main.rs modified"
  ]
}
```

---

## 10. Checkpointing

A projection records its processing position through checkpoints.

### Checkpoint Contract

- A checkpoint represents the sequence number of the last processed
  event.
- Checkpoints do not replace Event Store history.
- Checkpoint loss must allow rebuild through replay from sequence 0.
- Checkpoint updates are atomic with state updates (no partial
  progress).

### Persistence

Checkpoint persistence is intentionally not defined in this RFC. The
Projection Engine may store checkpoints in memory, on disk, or through
an external mechanism. The guarantee is that checkpoint loss triggers a
full rebuild, not data corruption.

---

## 11. Failure Semantics

### Projection Failure

When a projection fails during event processing:

- The projection is marked as Failed.
- The Event Store is unaffected.
- Derived state may be partially updated (invalid).
- Recovery requires a full rebuild.

### Invalid Event Handling

When a projection encounters an event it cannot process:

- The failure is isolated to that projection.
- No event mutation occurs.
- Other projections continue processing normally.
- The failed projection enters the Failed state.

### Rebuild Failure

When a projection fails during rebuild:

- Existing valid state is either preserved or explicitly invalidated.
- The Event Store remains authoritative.
- A retry may be attempted from the beginning of replay.

---

## 12. Invariants

Every conforming implementation MUST satisfy:

1. The Event Store is the source of truth.
2. Projection state is disposable.
3. Projection processing follows sequence order.
4. Projection rebuild is deterministic.
5. Projections cannot mutate the Event Store.
6. Projection failure cannot corrupt events.
7. Consumer contracts must not leak Event Store internals.

---

## 13. Non-Goals

The following are explicitly out of scope for this RFC:

- Filesystem watching
- Event creation
- Event Store persistence (see RFC-0001, ADR-0006)
- AI inference
- Execution engine
- Networking
- Plugins
- Distributed projection synchronization
