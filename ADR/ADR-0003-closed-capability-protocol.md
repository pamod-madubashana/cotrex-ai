# ADR-0003: Closed Capability Protocol

**Status:** Accepted

---

## Context

The protocol defines the interface between the kernel and AI inference providers. The set of capabilities must be controlled to ensure deterministic execution, protocol stability, and clear ownership boundaries.

An open capability system allows providers to introduce new function types at runtime. This creates unpredictability: the kernel cannot verify what capabilities exist, versioning becomes ambiguous, and testing requires covering unbounded combinations.

The protocol is owned by the kernel. Adding a capability is a protocol revision, not a plugin installation.

---

## Decision

**The capability set is closed.**

Providers cannot invent new capability types. The `CapabilityRequest` and `CapabilityResponse` enums define the complete set of supported capabilities. Adding a new capability requires modifying the protocol crate, incrementing the protocol version, and updating all implementations.

This decision is enforced at compile time. A provider implementing a capability not defined in the protocol will fail to compile.

---

## Alternatives Considered

### Open-Ended Function Calling

Allow providers to define arbitrary function signatures that the kernel invokes dynamically.

**Rejected because:**
- No type safety — requests and responses become untyped
- No versioning — function signatures can change without notice
- No deterministic execution — behavior depends on provider-specific definitions
- Testing requires covering unbounded combinations
- Violates protocol ownership — providers would own capability definitions

### Unrestricted Plugin Execution

Allow providers to register arbitrary capabilities at runtime without kernel awareness.

**Rejected because:**
- Kernel cannot verify what capabilities exist
- Security surface expands unpredictably
- No compile-time guarantees
- Behavior becomes non-deterministic across providers
- Breaks the boundary between protocol and implementation

---

## Consequences

### Positive

- **Type safety:** All capabilities are compile-time verified
- **Protocol stability:** Changes are explicit and versioned
- **Deterministic execution:** Kernel knows exactly what capabilities exist
- **Testability:** Fixed set of capabilities to test
- **Ownership clarity:** Kernel owns the protocol; providers implement it

### Negative

- **Rigidity:** Adding capabilities requires protocol evolution
- **Coordination cost:** All providers must update when capabilities change
- **No ad-hoc extensions:** Providers cannot add capabilities without kernel awareness

---

## References

- ARCHITECTURE.md: Protocol Ownership section
- Vision.md: Closed Capability System principle
- contract/src/lib.rs: CapabilityKind, CapabilityRequest, CapabilityResponse
- ADR-0002: Protocol Versioning Strategy
