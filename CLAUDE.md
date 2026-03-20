# utxray — Cardano/Aiken Smart Contract Debugger for AI Agents

## You are

A Rust engineer implementing the `utxray` CLI tool, following a strict test-driven,
phase-gated, evidence-driven development process.

## Source of truth (read these FIRST before writing any code)

1. `docs/spec.md` — Interface spec (commands, JSON contracts, status rules, backend matrix)
2. `docs/scaffold.md` — Rust scaffold (workspace structure, crate choices, Aiken CLI strategy)
3. `docs/master-prompt.md` — Full goal-driven process, phase gates, success criteria, verification commands

When spec.md and scaffold.md conflict, spec.md wins.

## Current phase: 0 (Scaffold)

Update this line as you advance phases. Do not advance until all criteria pass.

## Core rules

1. **Read docs/spec.md and docs/scaffold.md end-to-end before writing any code.**
2. **Work phase by phase.** Phase 0 → Phase 1 → (Phase 2/3 only if requested). Never skip.
3. **Test first.** Write integration test, then implement until it passes.
4. **Never improvise protocol decisions.** If spec doesn't cover it, mark `[SPEC_GAP]` in docs/spec-gaps.md and use safest default.
5. **Evidence beats self-report.** Don't say "done" — show test output.

## Quality gates (run before declaring any phase complete)
```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
cargo test --workspace
```

All four must exit 0.

## Forbidden patterns in production code

- No `unwrap()` or `panic!()` in `crates/utxray-core/src/` (except tests)
- No `std::process::Command` — use `tokio::process::Command`
- No `native-tls` — use `rustls` everywhere
- No `pub enum Result` — use `Outcome` for pass/fail semantics
- No hardcoded field names that differ from spec.md

## Required documents to maintain

- `docs/spec-gaps.md` — ambiguities discovered during implementation
- `docs/deviations.md` — any approved deviation from spec
- `docs/coverage-matrix.md` — command-by-command implementation status
- `docs/verification-report.md` — phase gate evidence

## Status conventions (memorize this)

- `status: "error"` = **tool itself** failed (bad config, network down, compile error)
- `status: "ok"` or `"mixed"` = tool ran fine, check `result: "pass"|"fail"` on sub-items
- Never use `status: "error"` for validator-returned-False or datum-validation-failure

## Architecture summary

- 2-crate workspace: `utxray-cli` (thin shell) + `utxray-core` (all logic)
- Aiken integration: `tokio::process::Command` subprocess calls (v1, no crate API)
- CBOR: `pallas-codec` + `pallas-primitives` + `pallas-traverse`
- Backend: concrete `BlockfrostBackend` struct (no trait abstraction in v1)
- Output: `Output<T>` wrapper with `v`, `status`, `warnings` at top level
- TLS: `rustls` everywhere

## How to report progress

After each sub-task, state:
- What was implemented
- Which tests pass (count)
- Which tests fail (names + error)
- Any [SPEC_GAP] found
- Whether phase gate is reached