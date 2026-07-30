# ADR-0005: AI as Advisory Layer

**Status:** Accepted

---

## Context

AI models are probabilistic. They produce outputs that may vary across invocations, may be incorrect, and cannot guarantee deterministic behavior. The kernel, by contrast, must maintain deterministic, verifiable state.

If AI models directly mutate application state, correctness becomes unverifiable. There is no audit trail for AI-initiated changes, no mechanism to reject incorrect inferences, and no way to replay state without the model.

The kernel is the source of truth. AI is a consumer of that truth, not an authority over it.

---

## Decision

**AI is advisory only.**

The kernel owns project state. The Intelligence Brain orchestrates AI capabilities but delegates execution to the cotrex-ai runtime. The runtime performs inference and returns structured responses. The kernel decides whether to apply those responses.

AI proposes. Kernel decides.

No AI model directly owns or mutates application state. All state changes flow through the kernel's event sourcing system, maintaining a complete audit trail and enabling deterministic reconstruction.

---

## Alternatives Considered

### AI Directly Mutates Application State

Allow AI models to execute arbitrary state changes without kernel mediation.

**Rejected because:**
- Non-deterministic — model outputs vary across invocations
- No audit trail — state changes bypass the event store
- No rejection mechanism — kernel cannot refuse incorrect inferences
- Violates "Kernel Owns Reality" principle
- State reconstruction requires the model, not just events

---

## Consequences

### Positive

- **Deterministic kernel:** State changes are recorded as events, not model outputs
- **Safety:** Kernel can validate and reject AI proposals before application
- **Auditability:** Complete history of what AI proposed and what the kernel decided
- **Provider independence:** Replacing the model never requires changing the kernel
- **Replayability:** State can be reconstructed from events alone, without the model

### Negative

- **Additional orchestration layer:** Intelligence Brain mediates between kernel and runtime
- **Latency:** AI responses require kernel validation before application
- **Indirection:** AI cannot act autonomously; all actions are mediated

---

## References

- ARCHITECTURE.md: Layer 1 (Kernel), Layer 2 (Intelligence Brain), Architectural Invariants
- Vision.md: Kernel Owns Reality, AI Is A Consumer principles
