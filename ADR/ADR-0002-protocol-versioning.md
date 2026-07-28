# ADR-0002: Protocol Versioning Strategy

**Status:** Accepted

---

## Context

The cotrex-ai protocol defines the interface between the kernel and AI inference providers. The protocol version must be managed in a way that ensures correctness while avoiding unnecessary complexity.

The protocol is intentionally closed. Providers cannot invent new capability types. Adding a capability is a protocol revision.

---

## Decision

**Exact protocol version match is required.**

A provider implementing protocol version 1.0 will reject requests tagged 1.1 and vice versa. There is no negotiation, no downgrade, and no compatibility layer.

Breaking changes are explicit and require a major version bump. Non-breaking additions require a minor version bump.

---

## Alternatives Considered

### Semantic Version Negotiation

Allow providers to advertise supported version ranges. The runtime negotiates the highest compatible version.

**Rejected because:**
- Adds complexity to the runtime
- Creates ambiguity about which version is "correct"
- Makes testing harder (many version combinations)
- The protocol is intentionally closed; negotiation implies openness

### Backward Compatibility

Allow newer providers to handle older protocol versions.

**Rejected because:**
- Creates hidden compatibility layers
- Makes behavior unpredictable
- Providers would need to maintain multiple code paths
- The protocol is small enough that version upgrades are cheap

### No Versioning

Use a single, unversioned protocol.

**Rejected because:**
- Provides no way to detect incompatibilities
- Makes evolution impossible without breaking changes
- No mechanism for gradual migration

---

## Consequences

### Positive

- **Simplicity:** No negotiation logic, no compatibility layers
- **Correctness:** Incompatible versions fail immediately, not silently
- **Testability:** One version to test, not a matrix
- **Evolution:** Protocol changes are explicit and versioned

### Negative

- **Coordination:** All components must upgrade together
- **No graceful degradation:** Incompatible versions cannot communicate
- **Migration burden:** Version bumps require updating all providers

---

## Trade-offs

| Aspect | Exact Match | Negotiation |
|--------|-------------|-------------|
| Complexity | Low | High |
| Correctness | High | Medium |
| Flexibility | Low | High |
| Testing | Simple | Complex |
| Migration | Coordinated | Gradual |

The trade-off favors correctness and simplicity over flexibility. The protocol is small and controlled; the coordination cost is acceptable.

---

## Future Review Trigger

This ADR must be revisited once:

1. More than one real provider exists in production
2. Multiple protocol versions are deployed simultaneously
3. A use case for gradual migration emerges

Until then, exact match remains the decision.

---

## References

- ARCHITECTURE.md: Protocol Ownership section
- Vision.md: Closed Capability System principle
- AGENTS.md: Protocol Types section
