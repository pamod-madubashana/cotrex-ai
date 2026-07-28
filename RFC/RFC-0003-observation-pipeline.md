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

## 2. Glossary

- **Observation**: a raw notification from an external system indicating
  a potential change.
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

## 3. Architecture Position

The Observation Pipeline sits between the external world and the Event
Store.

```text
External World
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
Derived State
```

The pipeline owns:

- external observation (filesystem watching, process monitoring)
- raw event filtering (ignore irrelevant changes)
- event translation (raw notification → typed event)
- EventStore append requests

The pipeline does NOT own:

- sequence number assignment (Event Store)
- event ordering (Event Store)
- projection updates (Projection Engine)
- AI context generation (AI Context Projection)
- derived state
- execution
- AI inference

---

## 4. Observation Guarantees

### 4.1 Event Creation

Every accepted observation produces exactly one committed event.

If the Event Store rejects the append (backpressure, failure), the
observation is not considered observed. The pipeline must propagate the
error.

### 4.2 Ordering

The Observation Pipeline does not assign sequence numbers. The Event
Store is the sole owner of event ordering.

The pipeline may observe changes in any order. The Event Store commits
them in arrival order. Projections consume them in sequence order.

### 4.3 No Semantic Interpretation

The pipeline translates raw observations into typed events. It does not:

- interpret file contents
- analyze change semantics
- detect conflicts
- merge changes
- reason about project structure

Semantic understanding belongs to projections and the Intelligence Brain.

---

## 5. Event Translation

The pipeline translates raw observations into `FileChanged` events.

### 5.1 FileChanged Event

From RFC-0001:

```rust
FileChanged {
    path: PathBuf,
    operation: FileOperation,
    timestamp: SystemTime,
}
```

### 5.2 Translation Rules

| Raw Observation | Translated Event |
|-----------------|------------------|
| File created | `FileChanged { operation: Created }` |
| File modified | `FileChanged { operation: Modified }` |
| File deleted | `FileChanged { operation: Deleted }` |
| File renamed | `FileChanged { operation: Deleted }` + `FileChanged { operation: Created }` |
| Metadata changed | Ignored (not a file content change) |

### 5.3 Timestamp

The `timestamp` field records when the observation was translated, not
when the filesystem change occurred. Wall-clock time is not authoritative
for ordering; sequence numbers are.

---

## 6. File Watching Semantics

### 6.1 Recursive Watching

The MVP watches the project root recursively. All subdirectories are
observed.

### 6.2 Ignore Patterns

The following are ignored by default:

- `.git/` directory
- `target/` directory
- Editor temporary files (files starting with `.` or ending with `~`)
- Binary files (optional, configurable)

### 6.3 Scope

The pipeline watches:

- file creation
- file modification
- file deletion

The pipeline does NOT watch:

- process execution
- network changes
- environment variable changes
- socket activity

---

## 7. Filtering Rules

### 7.1 Accepted Observations

An observation is accepted if:

- the path is within the project root
- the path does not match ignore patterns
- the operation is a recognized filesystem change

### 7.2 Rejected Observations

An observation is rejected if:

- the path is outside the project root
- the path matches ignore patterns
- the operation is metadata-only (chmod, chown)
- the observation is a duplicate raw notification (see Section 8)

### 7.3 Filtering Errors

Filtering is deterministic. The same observation always produces the same
decision. Filtering does not depend on:

- project state
- previous observations
- time of day
- external configuration

---

## 8. Duplicate Notification Policy

### 8.1 MVP Behavior

The MVP does NOT guarantee semantic deduplication.

A raw filesystem notification produces an event. If the filesystem sends
duplicate notifications for the same logical change, the pipeline
produces duplicate events.

The Event Store preserves all accepted observations.

### 8.2 Rationale

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

### 8.3 Future Work

Semantic deduplication is deferred to a future RFC:

- RFC-0006: Observation Normalization

Potential strategies:

- content hashing
- time-window coalescing
- path-based deduplication
- editor-aware normalization

---

## 9. Failure Semantics

### 9.1 Watcher Failure

If the filesystem watcher crashes or becomes unavailable:

- the pipeline enters the `Failed` state
- the Event Store remains valid
- existing committed events remain available
- new observations are not processed until recovery

### 9.2 Append Failure

If `EventStore.append()` fails:

- the event is not considered observed
- the failure is propagated to the caller
- no silent loss occurs
- the pipeline may retry or enter `Failed` state

### 9.3 Translation Failure

If a raw observation cannot be translated into a valid event:

- the observation is rejected
- the rejection is logged
- the pipeline continues processing other observations
- no phantom events are created

---

## 10. Backpressure Handling

If the Event Store cannot accept events (capacity reached):

Allowed:

- block the producer until capacity is available
- return an explicit error to the caller

Forbidden:

- dropping filesystem events silently
- silently discarding observations
- buffering without bound

The pipeline must either deliver the event or report failure. Never
silent loss.

---

## 11. Lifecycle

### 11.1 States

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

### 11.2 Allowed Transitions

| From | To | Trigger |
|------|----|---------|
| Created | Initializing | Start requested |
| Initializing | Watching | Watcher successfully attached |
| Watching | Failed | Watcher error or append failure |
| Failed | Initializing | Recovery requested |
| Watching | Stopped | Shutdown requested |

### 11.3 Invalid Transitions

- Created → Watching (must initialize first)
- Stopped → Watching (cannot restart after shutdown)
- Failed → Watching (must reinitialize first)

---

## 12. Invariants

Every conforming implementation MUST satisfy:

1. Observation creates events, not state.
2. The Event Store is the only owner of event ordering.
3. Every accepted observation becomes exactly one committed event.
4. Failed observations never create phantom events.
5. Projections never receive raw watcher events.
6. Duplicate raw notifications are preserved unless future normalization
   exists.
7. Observation failure cannot corrupt Event Store history.
8. The pipeline does not interpret file contents.
9. The pipeline does not update projections or derived state.

---

## 13. Non-Goals

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
