# RFC-0001: Kernel Event Store

**Status:** Implemented
**Milestone:** 4
**Depends on:** ARCHITECTURE.md, ADR-0002 (Protocol Versioning Strategy)

---

## 1. Purpose

This RFC defines the Kernel Event Store: the subsystem responsible for
durable, append-only storage of kernel events.

It specifies **behavioral guarantees only**. It intentionally does not
specify:

- storage engine
- database
- serialization format
- filesystem layout
- optimization strategy

Any implementation satisfying the guarantees in this document is a
conforming Event Store, regardless of internal design.

---

## 2. Glossary

- **Event**: an immutable record of something that happened.
- **Append**: the act of atomically writing and committing an event.
- **Committed**: an event is committed once it has been durably written
  and assigned a sequence number. Only committed events are visible to
  replay or projections. An event that fails to commit does not exist
  from the perspective of any consumer.
- **Replay**: deterministic iteration over committed events, in sequence
  order.
- **Projection**: derived state rebuilt from replay.

---

## 3. Event Ordering Guarantees

### 3.1 Global Ordering

Every accepted event receives a monotonically increasing sequence number.

```
1, 2, 3, 4, 5, ...
```

Sequence numbers:

- never repeat
- never decrease
- are immutable once assigned
- define the canonical event order

Arrival time (wall-clock time) never determines ordering. Only the
sequence number does.

### 3.2 Ordering Rule

```
Append A
Append B

Result: A.sequence < B.sequence
```

Replay MUST always return `A, B` in that order — never `B, A`.

---

## 4. Write Ordering

### 4.1 Atomic Append Semantics

```
append(event) → durable → visible
```

The reverse ordering (`visible → durable`) is forbidden.

### 4.2 Required Guarantees

- Append is atomic — the event is either fully written or not written at all.
- Partial writes are impossible.
- A failed append produces no visible event.
- Every accepted append receives exactly one sequence number.
- **Sequence numbers are allocated only after an append has been durably
  committed.** Implementations MUST NOT reserve a sequence number ahead
  of a write and later discard it — every allocated sequence number
  corresponds to exactly one committed event, with no gaps left by
  aborted or failed appends.

---

## 5. Replay Guarantees

Replay MUST be deterministic.

### 5.1 Contract

```
Replay(start_sequence) → returns all events where sequence >= start_sequence
```

Replay MUST:

- preserve order
- never skip a committed event
- never duplicate an event

### 5.2 Snapshot Semantics

Replay is a **bounded, point-in-time snapshot**, not a live stream.

> Replay observes a consistent snapshot of the log as of the moment
> replay begins, defined by the highest committed sequence number at
> that moment (`snapshot_end`). Replay terminates after returning the
> event at `snapshot_end`. Events appended after replay starts are not
> required to appear in that replay, even if the append completes before
> replay terminates.

Live/streaming consumption of newly appended events (i.e. subscribing
rather than replaying) is out of scope for this RFC and, if needed, will
be defined separately — this RFC governs bounded replay only.

### 5.3 Example

Given committed events `1, 2, 3, 4, 5`, `Replay(3)` returns exactly
`3, 4, 5`.

---

## 6. Projection Consistency

Projections are downstream consumers. They never define correctness.

```
Event Store → Projection      (correct)
Projection → Event Store      (forbidden)
```

### 6.1 Guarantee

Every committed event MUST become available to every registered
projection, unless that projection has failed (see §9, Failure
Semantics). This RFC does not define delivery latency, delivery
mechanism, or ordering across projections — those are left to a future
RFC (e.g. RFC-0002, Projection Engine).

### 6.2 Invariant

Projection state is disposable. A projection can always be rebuilt from
replay, from sequence 0, with no loss of correctness.

---

## 7. Backpressure

This section defines required behavior, not a required mechanism.

```
Producer → Event Store → Queue → Projection
```

Possible implementations include a bounded queue, a blocking producer, or
an async wait — this RFC does not choose between them.

### 7.1 Requirement

> If the system cannot safely accept additional events, append MUST fail
> or block. Silent event loss is forbidden under all circumstances.

---

## 8. FileChanged Event (MVP Scope)

Per the locked Kernel MVP scope, the only event type in this RFC is:

```
FileChanged {
    path: PathBuf,
    operation: FileOperation,   // Created | Modified | Deleted
    timestamp: SystemTime,
}
```

Explicitly excluded from this event: content hashes, git metadata, AI
annotations, or any other metadata not required to represent "a file
changed."

---

## 9. Failure Semantics

| Failure mode          | Required behavior                          |
|------------------------|---------------------------------------------|
| Append failure          | Event is not stored; no sequence number consumed |
| Replay failure           | No ordering corruption; replay may be retried from a known-good sequence |
| Projection failure        | Event Store is unaffected; projection alone is invalid until rebuilt |
| Storage corruption         | Implementation-defined recovery, but MUST NOT silently reorder or fabricate events |

---

## 10. Event Identity and Envelope

Event identity is distinct from event ordering.

```rust
struct Event {
    id: Uuid,                   // identity — constant across copies/replays
    sequence: u64,                // ordering — assigned once, at commit time
    occurred_at: SystemTime,       // informational only — see below
    payload: EventPayload,
}

enum EventPayload {
    FileChanged(FileChanged),
}
```

- `sequence` defines order.
- `id` defines identity, and is stable across replication, replay, or
  logging.
- **`occurred_at` MUST NOT influence ordering, replay order, or conflict
  resolution in any implementation.** It is informational metadata only.
  Any logic that sorts, deduplicates, or resolves conflicts using
  `occurred_at` is non-conforming, regardless of how reasonable it seems
  at the time.

Conflating identity and ordering is a common source of subtle bugs in
event store implementations (e.g. treating a replayed event as "new"
because it was reassigned a fresh identifier) and is disallowed by this
RFC.

The `EventPayload` enum is the extension point for future event types.
The MVP defines exactly one variant, `FileChanged`; later milestones add
variants to this enum rather than changing the `Event` envelope itself.

---

## 11. Invariants

Every conforming implementation MUST satisfy:

1. The Event Store is append-only.
2. Sequence numbers are strictly increasing.
3. Replay is deterministic.
4. Committed events are immutable.
5. Projection state is rebuildable from replay alone.
6. Accepted events are never silently dropped.
7. Ordering is defined solely by sequence number, never by arrival time.

---

## 12. Non-Goals

The following are explicitly out of scope for this RFC and for Milestone 4:

- Git integration
- Build system events
- AI/inference-related events
- Execution or command events
- Search or knowledge graph events
- Networking or distributed replication
- Plugin-defined event types

These may be addressed in future RFCs once the Event Store's core
contract is proven.

---

## 13. MVP Storage Limitation

The Milestone 4 implementation provides durability within a single
process lifetime only: committed events survive for as long as the
process is running, but are not persisted across process restart or
crash.

This does not weaken guarantees defined in sections 3–11.
Ordering, atomicity, replay determinism, and failure semantics remain
valid within the process lifetime.

Cross-restart persistence through disk-backed storage, write-ahead log,
or equivalent storage backend is explicitly deferred to a future ADR/RFC
(see ADR-0006: Event Store Persistence Strategy).

The Milestone 4 implementation MUST NOT be described as production
durable storage until persistent recovery is implemented.

### 13.1 Verification

- Lock ordering: events → next_sequence (consistent)
- Panic safety: no unwraps in public API paths (verified)
- Persistence: in-memory only (`Mutex<Vec<Event>>`)
