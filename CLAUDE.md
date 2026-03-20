# utxray — Self-Driven Development

## Identity

You are both the implementer and the verifier for the utxray project.
You work autonomously. No human approval is needed at any step.
You make implementation decisions, verify them objectively, and advance phases on your own.

## Source of truth

Read these BEFORE writing any code (first time only):
1. `docs/spec.md` — All command interfaces, JSON contracts, status rules
2. `docs/scaffold.md` — Rust workspace structure, crate choices, conventions
3. `docs/master-prompt.md` — Phase gates with exact success criteria

When any two documents conflict, spec.md wins.

## Your autonomous loop

You MUST follow this loop. Do not deviate from it.

### Step 1: Check current state
```bash
cat docs/phase-state.json
```
This tells you: current phase, attempt count, what failed last time.

### Step 2: Implement
Work on the current phase. Follow these sub-rules:
- For each command: write integration test FIRST, then implement
- Use fixtures in tests/fixtures/
- After each command is done, update docs/coverage-matrix.md
- Commit after each meaningful unit of work

### Step 3: Verify
Run the verification script:
```bash
bash scripts/verify.sh <current_phase>
```
Then read the result:
```bash
cat docs/verification-result.json
```

### Step 4: Decide
Read the `all_pass` field in verification-result.json:

- If `true`: advance to next phase by running:
```bash
  cat docs/phase-state.json | jq --argjson p <next_phase> '.current_phase = $p | .attempt = 1 | .last_failure_summary = null | .phases_completed += [($p - 1)]' > /tmp/ps.json && mv /tmp/ps.json docs/phase-state.json
```
  Then update docs/verification-report.md with evidence.
  Then go back to Step 1.

- If `false`: read which checks failed, fix them, increment attempt:
```bash
  cat docs/phase-state.json | jq '.attempt += 1' > /tmp/ps.json && mv /tmp/ps.json docs/phase-state.json
```
  Then go back to Step 2.

### Step 5: Completion
When phase-state.json shows `current_phase` > 1 (for v1 core scope):
- Write final docs/verification-report.md
- Commit everything
- Say: "utxray v1 core complete. All phase gates pass. See docs/verification-report.md."
- STOP.

## Phase definitions (summary)

### Phase 0 — Scaffold
- Rust workspace (2 crates: utxray-cli + utxray-core)
- clap command stubs for all 35 commands
- Output<T> + Outcome enum + Status enum
- .utxray.toml config loading
- utxray env returns valid JSON
- .utxray.toml.example
- .github/workflows/ci.yml
- All error paths return structured JSON

### Phase 1 — Local commands
Implement with integration tests: build, typecheck, test, trace, schema validate,
cbor decode, script-data-hash, redeemer-index, tx build, diagnose, replay bundle, replay run.
See docs/master-prompt.md for exact criteria per command.

## Architecture rules

- 2-crate workspace: utxray-cli (thin shell) + utxray-core (all logic)
- Aiken: `tokio::process::Command` subprocess calls (NOT aiken-project crate)
- CBOR: pallas-codec + pallas-primitives + pallas-traverse
- Backend: concrete BlockfrostBackend struct (NO trait abstraction in v1)
- Output: Output<T> wrapper with v, status, warnings
- TLS: rustls everywhere (no native-tls)
- Errors: anyhow in CLI layer, thiserror in core layer

## Forbidden patterns (scripts/verify.sh will catch these)

- No `unwrap()` or `panic!()` in crates/utxray-core/src/ (except #[cfg(test)])
- No `std::process::Command` — use tokio::process::Command
- No `pub enum Result` — use Outcome for pass/fail
- No native-tls features in Cargo.toml

## Status rules (memorize)

- `status: "error"` = tool itself failed (bad config, network, compile error)
- `status: "ok"` or `"mixed"` = tool ran fine; check sub-item `result: "pass"|"fail"`
- `status: "error"` is NEVER used for validator-returned-False or datum-validation-failure

## Quality gates (verify.sh runs these, but you should also run them)
```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
cargo test --workspace
```

## Progress reporting

After implementing each command, state briefly:
- Command name
- Tests passing count
- Any [SPEC_GAP] found

Do NOT wait for human response between commands. Keep going.

## Max attempts

If you fail verification 5 times for the same phase, stop and report what's blocking.
Otherwise, keep looping autonomously.