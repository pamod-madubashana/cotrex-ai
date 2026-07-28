# RFC-0004: Execution Engine

**Status:** Draft
**Milestone:** 7
**Depends on:** RFC-0001 (Kernel Event Store)

---

## 1. Purpose

This RFC defines the Execution Engine: the subsystem responsible for
performing actions in the external world and recording what happened as
immutable events.

The Execution Engine is where Cotrex stops being a historian and starts
touching the real machine. Observation tells Cotrex what happened.
Execution makes something happen. Both communicate through events.

The Event Store remains the source of truth. Execution creates events
about what it did; it does not interpret, summarize, or derive state from
them.

---

## 2. Glossary

- **Execution**: the act of performing an action in the external world
  (running a command, writing a file, making an API call).
- **ExecutionRequest**: a typed description of an action to perform.
- **ExecutionResult**: the outcome of an execution attempt.
- **Execution Engine**: the subsystem that receives requests, performs
  actions, and emits events.
- **Capability**: a declared ability an execution request requires
  (e.g., `command.run`, `file.write`).
- **Working Directory**: the filesystem context in which execution
  occurs.

---

## 3. Architecture Position

The Execution Engine sits between the AI Runtime and the external world.
It writes results back to the Event Store as immutable facts.

```text
                 AI Runtime Brain
                       |
                       v
               Execution Engine
                       |
          +------------+------------+
          v                         v
     External World            Event Store
```

### Ownership Rules

**Execution Engine owns:**

- receiving execution requests
- validating permissions and capabilities
- performing actions in the external world
- creating execution events
- calling EventStore.append()

**EventStore owns:**

- event identity assignment (UUID)
- sequence number allocation
- ordering guarantees
- committed event storage
- replay

**Execution Engine MUST NOT:**

- assign sequence numbers
- assign event IDs
- update projections
- create AI context
- interpret file contents
- decide what to execute (that is the AI Runtime's job)

---

## 4. Execution Lifecycle

### States

```text
Created
   |
   v
Queued
   |
   v
Running
   |
   +----------+
   |          |
   v          v
Completed  Failed
```

| State | Description |
|-------|-------------|
| Created | Execution request received but not yet queued. |
| Queued | Request is waiting for resources or scheduling. |
| Running | Action is being performed in the external world. |
| Completed | Action finished successfully. Terminal state. |
| Failed | Action finished with error. Terminal state. |

### Allowed Transitions

| From | To | Trigger |
|------|----|---------|
| Created | Queued | Request accepted into queue |
| Queued | Running | Resources available, execution begins |
| Running | Completed | Action finished successfully |
| Running | Failed | Action failed or was interrupted |

### Invalid Transitions

- Created → Running (must queue first)
- Queued → Completed (must run first)
- Completed → anything (terminal)
- Failed → anything (terminal)

### Future States

These states are explicitly deferred:

- **Cancelled**: execution cancelled before or during run
- **TimedOut**: execution exceeded time limit

They are NOT part of this RFC. They will be introduced when production
usage justifies them.

---

## 5. Execution Events

### EventPayload Variants

The Execution Engine introduces three new EventPayload variants:

```rust
EventPayload::ExecutionRequested(ExecutionRequested)
EventPayload::ExecutionCompleted(ExecutionCompleted)
EventPayload::ExecutionFailed(ExecutionFailed)
```

### ExecutionRequested

```rust
pub struct ExecutionRequested {
    pub execution_id: Uuid,
    pub command: String,
    pub working_directory: PathBuf,
    pub requested_at: SystemTime,
}
```

Emitted when the Execution Engine accepts a request. Records what was
asked, not what happened.

### ExecutionCompleted

```rust
pub struct ExecutionCompleted {
    pub execution_id: Uuid,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub completed_at: SystemTime,
}
```

Emitted when execution finishes successfully. Records outcome metadata.
Does NOT store stdout/stderr content.

### ExecutionFailed

```rust
pub struct ExecutionFailed {
    pub execution_id: Uuid,
    pub error: String,
    pub duration_ms: u64,
    pub failed_at: SystemTime,
}
```

Emitted when execution fails. Records the error reason. Does NOT store
full output dumps.

### Output Storage Rule

Large execution output (stdout, stderr, compiler dumps) MUST NOT be
stored in events. Events store references and metadata only.

```
Allowed:
  execution_id, exit_code, duration_ms, error summary

Forbidden:
  stdout: 50000 lines
  stderr: giant compiler dump
  binary output
```

Rationale: The Event Store is append-only. Storing large payloads
destroys replay performance and memory usage. Large data belongs in
external storage referenced by execution_id.

---

## 6. Security Boundary

### Permission Model

The Execution Engine does NOT accept arbitrary commands. Every
execution request must declare its required capabilities.

```rust
pub struct ExecutionRequest {
    pub command: String,
    pub working_directory: PathBuf,
    pub required_capabilities: Vec<Capability>,
}
```

### Capability Enumeration

This RFC defines the initial capability set. The set is closed; adding
new capabilities is a protocol revision.

| Capability | Description |
|------------|-------------|
| `command.run` | Execute shell commands |
| `file.write` | Create or modify files |
| `file.delete` | Remove files |

### Policy Enforcement

The Execution Engine validates capabilities before execution:

1. Request declares required capabilities.
2. Engine checks if caller holds those capabilities.
3. If valid → execute.
4. If invalid → reject with error, emit no events.

The exact policy mechanism (allowlist, denylist, permission grants) is
an implementation detail. The contract is: **invalid requests are
rejected before execution begins.**

### Blocked by Default

Commands that match dangerous patterns are blocked unless explicitly
allowed:

- `rm -rf /`
- `curl ... | bash`
- `chmod 777`
- Commands exceeding path depth limits

The specific patterns are implementation-defined. The principle is:
the Engine must have a reason to say yes, not a reason to say no.

---

## 7. EventStore Integration

### Append Flow

```text
ExecutionRequest
      |
      v
Validate capabilities
      |
      v
ExecutionRequested → EventStore.append()
      |
      v
Perform action in external world
      |
      +---> Success: ExecutionCompleted → EventStore.append()
      |
      +---> Failure: ExecutionFailed → EventStore.append()
```

### Error Handling

If `EventStore.append()` fails during event creation:

- The execution is not considered observed
- The failure is propagated to the caller
- No silent loss occurs
- The Execution Engine remains operational

If the external action fails:

- ExecutionFailed event is created
- The failure is recorded as an immutable fact
- The Engine continues processing future requests

---

## 8. Failure Semantics

### Execution Failure

If the external action fails (non-zero exit code, crash, timeout):

- ExecutionFailed event is appended
- The error is recorded in the event
- The Engine does not retry automatically
- The AI Runtime decides whether to retry

### Append Failure

If EventStore.append() fails:

- The event is not committed
- The failure is propagated to the caller
- No partial event exists

### Metadata Transactionality

Execution is transactional around metadata, not world state.

**Guaranteed:**

```
Requested → Started → Completed / Failed
```

**NOT guaranteed:**

```
Started → Rollback world → Everything exactly like before
```

Most commands are not reversible. RFC-0004 does not pretend rollback
exists. If the world is changed, it is changed. The event records what
happened.

---

## 9. Guarantees

### Event Creation

Every accepted execution request produces exactly one or more committed
events:

- ExecutionRequested at start
- ExecutionCompleted or ExecutionFailed at end

If the EventStore rejects the append, the request is not considered
observed. The pipeline must propagate the error.

### Ordering

The Execution Engine does not assign sequence numbers. The EventStore
is the sole owner of event ordering.

Multiple concurrent executions may interleave. The EventStore commits
them in arrival order. Projections consume them in sequence order.

### No Semantic Interpretation

The Execution Engine performs actions and records facts. It does not:

- interpret file contents
- analyze change semantics
- detect conflicts
- merge changes
- reason about project structure
- decide what to execute next

Semantic understanding belongs to the AI Runtime and projections.

---

## 10. Invariants

Every conforming implementation MUST satisfy:

1. Every accepted execution request becomes exactly one or more committed
   events.
2. The EventStore is the only owner of event ordering.
3. Failed executions never create phantom events.
4. Projections never receive raw execution output.
5. Execution failure cannot corrupt EventStore history.
6. The Engine does not interpret file contents.
7. The Engine does not update projections or derived state.
8. The Engine does not decide what to execute.
9. Large output is never stored in events.
10. Invalid capability requests are rejected before execution.

---

## 11. Non-Goals

The following are explicitly out of scope for this RFC:

- Automatic retry logic
- Rollback or undo mechanisms
- Output storage (stdout/stderr capture)
- Process monitoring
- Distributed execution
- Sandbox enforcement
- Resource limits (CPU, memory)
- Scheduling policies
- Queue persistence
- Command history UI
