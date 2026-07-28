# AGENTS.md — cotrex-ai

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

This is the **Cotrex AI Runtime** — a protocol-first system where the contract defines the protocol and the runtime executes it. No AI inference exists yet (Phase 1).

```
contract/     → protocol types only (no logic, no providers)
runtime/      → CapabilityProvider trait + extension methods
providers/    → provider implementations (mock only for now)
examples/     → usage examples (binary: example-runner)
```

**Key invariant:** The kernel depends on the protocol. The runtime depends on the protocol. Providers depend on the runtime. Models are implementation details.

## Error Split

- `contract::CapabilityError` — protocol-level errors (`InvalidRequest`, `UnsupportedProtocolVersion`)
- `runtime::RuntimeError` — execution errors (`Provider`, `InvalidResponse`, `Capability`)

Runtime errors wrap contract errors via `From` impl.

## Protocol Types

- `CapabilityRequest` (not `Capability`) — the request enum
- `CapabilityResponse` — the response enum
- `RequestMetadata` — contains `Uuid` + `SystemTime`, attach to every request
- `ProtocolVersion { major, minor }` — versioned, not a bare `u32`

## Verification

- Edition: 2024
- All protocol types derive `Debug, Clone, Serialize, Deserialize, PartialEq, Eq`
- `MockProvider` returns deterministic responses — no randomness, no AI
- `CapabilityProviderExt` provides ergonomic `.build_summary()` and `.explain_rust()` methods

## Tools

- **RTK** — all shell commands go through `rtk <cmd>` (not raw shell)
- **graphify** — use `graphify query/explain/path` for codebase exploration when `graphify-out/` exists
