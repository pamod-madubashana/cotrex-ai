# ADR-0004: Cargo Workspace

**Status:** Accepted

---

## Context

The system comprises multiple subsystems with distinct responsibilities: protocol types, runtime traits, kernel state management, execution orchestration, AI inference, and provider implementations. Each subsystem has different dependency requirements and architectural constraints.

Without physical separation, subsystems can develop circular dependencies, blurred boundaries, and implicit coupling. A single crate makes dependency direction unenforceable at compile time.

The architecture requires that dependencies flow in one direction: providers depend on the runtime, the runtime depends on the protocol. The kernel is independent.

---

## Decision

**The project uses a multi-crate Cargo workspace.**

Each architectural layer maps to a separate crate:

- `contract` — protocol types only, no logic
- `runtime` — capability provider traits and extension methods
- `kernel` — event-sourced state management
- `execution` — command execution orchestration
- `agent` — agentic loop and capability dispatch
- `capabilities` — capability response parsers
- `providers/*` — provider implementations

Crate boundaries enforce dependency direction at compile time. A crate that depends on another must declare the dependency explicitly. Circular dependencies are rejected by the compiler.

---

## Alternatives Considered

### Single Crate Repository

Place all subsystems in a single crate with module boundaries.

**Rejected because:**
- Module boundaries are not enforced — any module can import any other
- Dependency direction is a convention, not a compile-time guarantee
- Circular dependencies can develop silently
- Incremental compilation affects the entire crate
- No physical isolation between subsystems

---

## Consequences

### Positive

- **Compile-time enforcement:** Dependency direction is structural, not conventional
- **Physical isolation:** Crates cannot access private items of other crates
- **Incremental builds:** Changes to one crate rebuild only dependents
- **Clear boundaries:** Each crate has a single, defined responsibility
- **Testability:** Crates can be tested in isolation

### Negative

- **Overhead:** Managing multiple crates adds complexity
- **API design:** Public interfaces must be explicitly defined at crate boundaries
- **Coordination:** Changes across crate boundaries require updating multiple crates

---

## References

- ARCHITECTURE.md: Workspace Layout, Dependency Direction, Architectural Invariants
- Cargo.toml: workspace member definitions
