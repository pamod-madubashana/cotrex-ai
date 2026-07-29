# AGENTS.md — cotrex-ai

## Documentation Hierarchy

| Document | Purpose |
|----------|---------|
| `Vision.md` | Why Cotrex exists. Philosophy, goals, principles. No implementation. |
| `ARCHITECTURE.md` | **Canonical source of truth.** Subsystems, boundaries, dependency direction. |
| `RFC/` | How subsystems are implemented. Protocol definitions, APIs. |
| `ADR/` | Why technologies or designs were chosen. |

**Rule:** `ARCHITECTURE.md` is the single architectural source of truth. All other documents derive from it.

## Commands

```bash
rtk cargo fmt --all
rtk cargo clippy --workspace -- -D warnings
rtk cargo check --workspace
rtk cargo test --workspace
```

Run in order. `cargo fmt` must come first — clippy/check will fail on unformatted code.

Single crate: `rtk cargo test -p contract`

## Architecture

```
contract/     → protocol types only (no logic, no providers)
runtime/      → CapabilityProvider trait + extension methods
kernel/       → event-sourced kernel (EventStore, projections, observation pipeline)
providers/    → provider implementations (mock, json-fixture)
examples/     → usage examples (binary: example-runner)
fixtures/     → JSON response fixtures
```

**Key invariant:** The kernel depends on the protocol. The runtime depends on the protocol. Providers depend on the runtime. Models are implementation details.

**Layer model:**
- Kernel → Intelligence Brain → cotrex-ai Runtime → Providers
- No layer depends on layers below it.

**Kernel independence:** `kernel/` does NOT depend on `contract/` or `runtime/`. It has only `thiserror` and `uuid` as dependencies. It is a standalone crate.

## Error Split

- `contract::CapabilityError` — protocol-level errors (`InvalidRequest`, `UnsupportedProtocolVersion`)
- `runtime::RuntimeError` — execution errors (`Provider`, `InvalidResponse`, `Capability`)

Runtime errors wrap contract errors via `From` impl.

## Protocol Types

- `CapabilityRequest` — the request enum
- `CapabilityResponse` — the response enum
- `RequestMetadata` — contains `Uuid` + `SystemTime`, attach to every request
- `ProtocolVersion { major, minor }` — exact version match required, no negotiation
- `ProviderInfo` — provider metadata (name, version, capabilities)
- `ProviderHealth` — provider health status (`Healthy`, `Degraded`, `Unhealthy`)

## Provider Trait

```rust
pub trait CapabilityProvider: Send + Sync {
    fn info(&self) -> ProviderInfo;
    fn health(&self) -> ProviderHealth;
    fn execute(&self, request: CapabilityRequest) -> Result<CapabilityResponse, RuntimeError>;
}
```

- Providers are `Send + Sync` — runtime may hold `Arc<dyn CapabilityProvider>`.
- Prompt building is private to each provider.
- The API is synchronous. Inference is CPU-bound.

## Kernel Modules

- `event.rs` — `Event`, `EventPayload`, `FileChanged`, `FileOperation`
- `store.rs` — `EventStore` (in-memory, append-only, with backpressure)
- `projection.rs` — `FileChangeProjection`, `ProjectionStatus` lifecycle
- `ai_context.rs` — `AiContextProjection` (semantic summary for AI)
- `engine.rs` — `ProjectionEngine` (multi-projection coordination, failure isolation)
- `observation/pipeline.rs` — `ObservationPipeline` (filter → translate → append)
- `observation/filter.rs` — `ObservationFilter` (ignore patterns, dotfiles, temp files)
- `observation/translator.rs` — `Translator` (raw observations → event payloads)

## Verification

- Edition: 2024
- All protocol types derive `Debug, Clone, Serialize, Deserialize, PartialEq, Eq`
- `MockProvider` returns deterministic responses — no randomness, no AI
- `CapabilityProviderExt` provides ergonomic `.build_summary()` and `.explain_rust()` methods

## Report Requirement

After every implementation task is completed, generate a status report covering:
1. Test suite results (pass/fail counts per crate)
2. Any RFC or ADR changes
3. Files modified with line counts
4. Git status and recent commits
5. Dependency graph changes (if any)

## Tools

- **RTK** — all shell commands go through `rtk <cmd>` (not raw shell)
- **graphify** — use `graphify query/explain/path` for codebase exploration when `graphify-out/` exists
