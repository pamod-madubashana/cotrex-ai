# ADR-0006: Event Store Persistence Strategy

**Status:** Proposed

---

## Context

RFC-0001 defines Event Store behavioral guarantees but does not define a storage backend.

Milestone 4 provides an in-memory implementation satisfying ordering, append, replay, projection, and failure semantics within a process lifetime.

The current implementation does not survive process restart.

Verification evidence:
- `kernel/src/store.rs:42-46` — `EventStore` uses `Mutex<Vec<Event>>` and `Mutex<u64>`
- No `std::fs`, `File`, `OpenOptions`, `sled`, `rocksdb`, or `sqlite` imports found
- All state is held in process memory

---

## Decision

Deferred.

Future evaluation will decide between:

- Write-ahead log (WAL)
- Embedded database (sled, redb)
- External storage backend

Additional decisions required:

- Sync vs buffered writes
- Recovery process after crash
- Corruption handling
- Migration strategy

---

## Consequences

Until this ADR is accepted:

- Event Store data is not crash persistent.
- Projections must not assume events survive restart.
- Production durability claims are prohibited.

---

## Verification

Lock ordering is consistent:
- `append`: events lock → next_sequence lock
- No reverse ordering found

Panic safety is acceptable:
- `store.rs:119` — `events.last().unwrap()` — guarded by `is_empty()` check
- `projection.rs:90` — fixed: `.unwrap()` → `.ok().and_then()`
- All other unwraps are in test code or use `unwrap_or` patterns

Persistence is in-memory only:
- No filesystem operations
- No database imports
- No external storage

---

## References

- RFC-0001: Kernel Event Store
- ARCHITECTURE.md: Kernel Subsystems
