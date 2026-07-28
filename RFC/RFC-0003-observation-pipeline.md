# RFC-0003: Observation Pipeline

**Status:** Implemented
**Milestone:** 6
**Depends on:** RFC-0001 (Kernel Event Store)

---

## 1. Purpose

This RFC defines the Observation Pipeline: the subsystem responsible for
observing external system changes and transforming them into kernel
events.

The Observation Pipeline is the bridge between the external world and
the kernel. It watches for changes, filters irrelevant noise, translates
raw notifications into domain events, and submits them to the Event
Store.

The Event Store remains the source of truth. The Observation Pipeline
creates events; it does not interpret, summarize, or derive state from
them.

---

## 2. Scope

This RFC defines:

- Raw observation model
- Translation rules
- Filtering behavior
- Pipeline lifecycle
- Ownership boundaries

This RFC does NOT define:

- Filesystem watcher implementation (platform-specific)
- Semantic deduplication (future RFC-0006)
- Content analysis
- Git integration

---

## 3. Glossary

- **Observation**: a raw notification from an external system indicating
  a potential change.
- **RawObservation**: the typed representation of a filesystem
  notification before translation.
- **Translation**: the process of converting a raw observation into a
  typed kernel event.
- **Filtering**: the process of discarding observations that are
  irrelevant to the kernel.
- **Accepted Observation**: an observation that passes filtering and is
  submitted to the Event Store.
- **Rejected Observation**: an observation discarded during filtering.
- **Duplicate Notification**: multiple raw notifications for the same
  logical change.

---

## 4. Architecture Position

The Observation Pipeline sits between the external world and the Event
Store.

```text
External World (filesystem)
        |
        v
Observation Pipeline
        |
        v
Event Store
        |
        v
Projection Engine
        |
        v
Derived State / AI Context
```

### Ownership Rules

**Observation Pipeline owns:**

- receiving raw observations
- validating observations against ignore rules
- translating raw operations to typed events
- creating EventPayload values
- calling EventStore.append()

**EventStore owns:**

- event identity assignment (UUID)
- sequence number allocation
- ordering guarantees
- committed event storage
- replay

**Projection Engine owns:**

- state derivation from events
- replay
- checkpoints

**Observation Pipeline MUST NOT:**

- assign sequence numbers
- assign event IDs
- update projections
- create AI context
- interpret file contents

---

## 5. Raw Observation Model

### RawObservation

```rust
pub struct RawObservation {
    pub path: PathBuf,
    pub operation: RawOperation,
}
```

Raw observations are temporary pipeline inputs. They are NOT stored in
the EventStore. After translation, raw observations are discarded.

### RawOperation

```rust
pub enum RawOperation {
    Created,
    Modified,
    Deleted,
    Renamed { from: PathBuf },
}
```

| Variant | Description |
|---------|-------------|
| Created | New file appeared |
| Modified | File content changed |
| Deleted | File removed |
| Renamed | File moved or renamed (produces two events) |

---

## 6. Translation Rules

The Translator converts RawObservation into EventPayload values.

### Translation Map

| RawOperation | EventPayload |
|--------------|--------------|
| Created | `FileChanged { path, operation: Created, timestamp }` |
| Modified | `FileChanged { path, operation: Modified, timestamp }` |
| Deleted | `FileChanged { path, operation: Deleted, timestamp }` |
| Renamed { from } | `FileChanged { path: from, operation: Deleted, timestamp }` + `FileChanged { path, operation: Created, timestamp }` |

### Timestamp

The `timestamp` field records when the observation was translated, not
when the filesystem change occurred. Wall-clock time is not authoritative
for ordering; sequence numbers are.

### Translation Output

```rust
pub fn translate(observation: &RawObservation) -> Result<Vec<EventPayload>, TranslationError>
```

- Returns `Ok(vec![payload])` for Created, Modified, Deleted
- Returns `Ok(vec![delete_payload, create_payload])` for Renamed
- Returns `Err` only if the observation cannot be represented

---

## 7. Filtering Rules

The ObservationFilter determines which observations are accepted.

### Default Ignore Patterns

| Pattern | Reason |
|---------|--------|
| `.git` | Version control metadata |
| `target` | Build artifacts |
| `.DS_Store` | macOS metadata |
| `Thumbs.db` | Windows metadata |

### Additional Rejection Rules

| Condition | Reason |
|-----------|--------|
| Path outside project root | Not in scope |
| Hidden files (`.env`, `.config`) | Dotfile prefix |
| Temp files (`file~`) | Editor backup |
| Swap files (`file.swp`, `file.swo`) | Vim swap |

### Filter Contract

```rust
pub fn filter(&self, observation: &RawObservation) -> FilterDecision
```

Filtering is deterministic. The same observation always produces the
same decision. Filtering does NOT depend on:

- project state
- previous observations
- time of day
- external configuration

### Custom Patterns

```rust
pub fn add_ignore_pattern(&mut self, pattern: String)
```

Patterns are matched against path components, not full paths.

---

## 8. Duplicate Notification Policy

### MVP Behavior

The MVP does NOT guarantee semantic deduplication.

A raw filesystem notification produces an event. If the filesystem sends
duplicate notifications for the same logical change, the pipeline
produces duplicate events.

```text
OS emits:
  modify(file.rs)
  modify(file.rs)
  modify(file.rs)

Pipeline produces:
  FileChanged(file.rs, Modified)
  FileChanged(file.rs, Modified)
  FileChanged(file.rs, Modified)
```

No debounce. No semantic deduplication.

### Rationale

Semantic change detection is complex:

```text
save file
  |
  editor writes temp file
  |
  delete old file
  |
  rename temp file
  |
  chmod metadata
```

Is that 1 change? 3 changes? 5 changes?

The filesystem does not know. The watcher does not know. Pretending
otherwise creates false correctness.

### Future Work

Semantic deduplication is deferred to a future RFC:

- RFC-0006: Observation Normalization

Potential strategies:

- content hashing
- time-window coalescing
- path-based deduplication
- editor-aware normalization

---

## 9. EventStore Integration

### Append Flow

```text
RawObservation
    |
    v
Filter (accept/reject)
    |
    v
Translate (RawObservation -> Vec<EventPayload>)
    |
    v
EventStore.append(payload)  // for each payload
    |
    v
Committed Event (with sequence number)
```

### Error Handling

If `EventStore.append()` fails:

- The event is not considered observed
- The failure is propagated to the caller
- No silent loss occurs
- The pipeline remains in Watching state

---

## 10. Failure Semantics

### Watcher Failure

If the filesystem watcher crashes or becomes unavailable:

- The pipeline enters the `Failed` state
- The EventStore remains valid
- Existing committed events remain available
- New observations are not processed until recovery

### Append Failure

If `EventStore.append()` fails:

- The event is not considered observed
- The failure is propagated to the caller
- No silent loss occurs
- The pipeline may retry or enter `Failed` state

### Translation Failure

If a raw observation cannot be translated into a valid event:

- The observation is rejected
- The rejection is logged via PipelineStats
- The pipeline continues processing other observations
- No phantom events are created

---

## 11. Backpressure Handling

If the EventStore cannot accept events (capacity reached):

**Allowed:**

- Block the producer until capacity is available
- Return an explicit error to the caller

**Forbidden:**

- Dropping filesystem events silently
- Silently discarding observations
- Buffering without bound

The pipeline must either deliver the event or report failure. Never
silent loss.

---

## 12. Lifecycle

### States

```text
Created
   |
   v
Initializing
   |
   v
Watching
   |
   +-----> Failed
   |
   v
Stopped
```

| State | Description |
|-------|-------------|
| Created | Pipeline instance exists but has not started. |
| Initializing | Pipeline is setting up filesystem watcher. |
| Watching | Pipeline is actively observing changes. |
| Failed | Pipeline encountered an error; observations suspended. |
| Stopped | Pipeline has been shut down. |

### Allowed Transitions

| From | To | Trigger |
|------|----|---------|
| Created | Initializing | Start requested |
| Initializing | Watching | Watcher successfully attached |
| Watching | Failed | Watcher error or append failure |
| Failed | Initializing | Recovery requested |
| Watching | Stopped | Shutdown requested |

### Invalid Transitions

- Created → Watching (must initialize first)
- Stopped → Watching (cannot restart after shutdown)
- Failed → Watching (must reinitialize first)

---

## 13. Statistics

PipelineStats provides monitoring counters.

```rust
pub struct PipelineStats {
    pub accepted: u64,
    pub rejected: u64,
    pub events_created: u64,
}
```

| Counter | Description |
|---------|-------------|
| accepted | Observations that passed filtering |
| rejected | Observations discarded by filtering |
| events_created | Total EventPayload values appended |

Statistics are observational only. They MUST NOT influence:

- event ordering
- event creation
- filtering behavior
- projection state

---

## 14. Guarantees

### Event Creation

Every accepted observation produces exactly one or more committed events.

If the EventStore rejects the append (backpressure, failure), the
observation is not considered observed. The pipeline must propagate the
error.

### Ordering

The ObservationPipeline does not assign sequence numbers. The EventStore
is the sole owner of event ordering.

The pipeline may observe changes in any order. The EventStore commits
them in arrival order. Projections consume them in sequence order.

### No Semantic Interpretation

The pipeline translates raw observations into typed events. It does not:

- interpret file contents
- analyze change semantics
- detect conflicts
- merge changes
- reason about project structure

Semantic understanding belongs to projections and the Intelligence Brain.

---

## 15. Invariants

Every conforming implementation MUST satisfy:

1. Observation creates events, not state.
2. The EventStore is the only owner of event ordering.
3. Every accepted observation becomes exactly one committed event.
4. Failed observations never create phantom events.
5. Projections never receive raw watcher events.
6. Duplicate raw notifications are preserved unless future normalization
   exists.
7. Observation failure cannot corrupt EventStore history.
8. The pipeline does not interpret file contents.
9. The pipeline does not update projections or derived state.

---

## 16. Non-Goals

The following are explicitly out of scope for this RFC:

- Filesystem indexing
- AI inference
- Content analysis
- Git integration
- Semantic deduplication
- File hashing
- Execution triggers
- Remote observation
- Distributed watchers
- Editor-specific normalization
- Process monitoring
- Network observation
