# utxray

> **UTxO X-Ray — Cardano smart contract debugger for AI agents.**
> Build, test, trace, diagnose, and submit Cardano transactions — all through structured JSON output designed for LLM consumption.

[![CI](https://github.com/cauu/utxray/actions/workflows/ci.yml/badge.svg)](https://github.com/cauu/utxray/actions)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue)](./LICENSE)

---

## Why utxray?

Cardano smart contract development has a high debugging cost. Error messages from the node are opaque (`ScriptFailure`, `Phase2Error`), UTxO indexing is non-trivial, and there's no unified tool that an AI agent can invoke to systematically diagnose failures.

**utxray bridges this gap:**

- **Structured JSON output** — Every command returns machine-parseable JSON with `v`, `status`, and domain-specific fields. AI agents consume it directly.
- **Full transaction lifecycle** — `build → evaluate → sign → submit`, verified on Cardano Preview testnet with real on-chain transactions.
- **Aiken-native** — Built around the Aiken toolchain (`build`, `check`, `trace`), with CIP-0057 blueprint parsing and schema validation.
- **Offline-first** — 24 commands work locally without any chain connection. Chain commands use Blockfrost.
- **Error diagnosis** — `diagnose` classifies failures into actionable error codes with `suggested_commands` for automated fix loops.

---

## Quick Start

### Prerequisites

- **Rust** >= 1.85 (with `cargo`)
- **Aiken** >= 1.1.0 ([install](https://aiken-lang.org/installation-instructions))
- **Blockfrost API key** (for chain commands, [get one free](https://blockfrost.io/))

### Install

```bash
git clone https://github.com/cauu/utxray.git
cd utxray
cargo build --release
# Binary at target/release/utxray
```

### Configuration

Create `.utxray.toml` in your project root:

```toml
[network]
default = "preview"

[blockfrost]
project_id = "previewXXXXXXXXXXX"

[defaults]
format = "json"
```

### First run

```bash
# Check environment
utxray env

# Build your Aiken project
utxray build

# Run tests
utxray test

# Decode on-chain CBOR
utxray cbor decode --hex d8798344aabbccdd1903e81a004c4b40
```

---

## Commands

34 commands across 10 categories. All output structured JSON by default.

### Static Analysis

| Command | Description | Backend |
|---------|-------------|---------|
| `build` | Compile Aiken project, produce blueprint | Local (aiken) |
| `typecheck` | Type-check without full build | Local (aiken) |
| `schema validate` | Validate datum/redeemer against CIP-0057 blueprint | Local |
| `cbor decode` | Decode CBOR hex to human-readable JSON | Local |
| `cbor diff` | Structural diff between two CBOR values | Local |
| `script-data-hash` | Compute script data hash (Alonzo-spec CBOR encoding) | Local |
| `redeemer-index` | Show sorted input → redeemer index mapping | Local |

### Testing

| Command | Description | Backend |
|---------|-------------|---------|
| `test` | Run Aiken tests, parse results with exec_units | Local (aiken) |
| `trace` | Trace validator with custom datum/redeemer, build ScriptContext | Local (aiken) |
| `test-sequence` | Multi-transaction state machine tests | Local |
| `scaffold test` | Generate test stubs from blueprint schema | Local |
| `uplc eval` | Evaluate UPLC program directly | Local (aiken) |

### Transaction Lifecycle

| Command | Description | Backend |
|---------|-------------|---------|
| `tx build` | Build Conway-era CBOR transaction from spec | Local |
| `tx evaluate` | Get script ExUnits via evaluator | Blockfrost |
| `tx simulate` | Full phase-1 + phase-2 simulation | Blockfrost |
| `tx sign` | Sign with ed25519 key (.skey file) | Local |
| `tx submit` | Submit to network (mainnet safety guard) | Blockfrost |

### Chain Context

| Command | Description | Backend |
|---------|-------------|---------|
| `utxo query` | Query UTxOs at address | Blockfrost |
| `utxo diff` | Diff UTxO sets by tx or slot range | Blockfrost |
| `datum resolve` | Resolve datum by hash | Blockfrost |
| `context params` | Protocol parameters | Blockfrost |
| `context tip` | Current tip (slot, epoch, height) | Blockfrost |

### Diagnosis & Replay

| Command | Description | Backend |
|---------|-------------|---------|
| `diagnose` | Classify errors → actionable error codes + suggested commands | Local |
| `replay bundle` | Package failure for reproducible replay | Local |
| `replay run` | Re-execute a replay bundle | Local |
| `replay diff` | Diff two replay results (exec_units delta, trace diff) | Local |

### Performance

| Command | Description | Backend |
|---------|-------------|---------|
| `budget show` | Per-validator CPU/mem usage vs protocol limits | Local |
| `budget compare` | Before/after budget comparison with regression detection | Local |

### Blueprint & Context

| Command | Description | Backend |
|---------|-------------|---------|
| `blueprint show` | CIP-0057 blueprint overview | Local |
| `blueprint apply` | Apply parameters to parameterized validator | Local |
| `blueprint convert` | Convert to cardano-cli text envelope format | Local |
| `gen-context` | Generate `.utxray/context.json` for AI agents | Local |

### Workflow

| Command | Description | Backend |
|---------|-------------|---------|
| `auto` | Orchestrate build → test → trace → tx → diagnose flows | Mixed |
| `env` | Check tools, config, Blockfrost connectivity | Mixed |

---

## Output Protocol

Every command returns JSON with a consistent top-level structure:

```json
{
  "v": "0.1.0",
  "status": "ok",
  ...domain-specific fields...
}
```

### Status semantics

| `status` | Meaning | Agent action |
|----------|---------|--------------|
| `ok` | Command succeeded. Check sub-items for `result: "pass"\|"fail"` | Inspect results |
| `mixed` | Partial success (some tests pass, some fail) | Inspect failing items |
| `error` | **Tool itself** failed (bad config, network, compile error) | Don't retry — fix the cause |

**Key rule:** `status: "error"` is never used for validator-returned-False or datum validation failures. Those are `ok` or `mixed` with sub-item `result: "fail"`.

### Exit codes

- `status: "ok"` or `"mixed"` → exit code `0`
- `status: "error"` → exit code `1`

This enables shell-based agent orchestration: `utxray test && utxray diagnose`.

---

## AI Agent Workflow

utxray is designed for autonomous AI debugging loops:

```
1. utxray env                              → toolchain ready?
2. utxray build                            → compiles?
3. utxray test                             → which tests fail? traces?
4. utxray diagnose --from ./test-result    → error_code + confidence
5. (AI modifies code based on diagnosis)
6. utxray test --match "failing_test"      → fixed?
7. utxray trace --validator escrow.spend \  → probe with real data
        --purpose spend --datum '...' --redeemer '...'
8. utxray schema validate --validator ...  → encoding correct?
9. utxray tx build --spec tx.json          → draft transaction
10. utxray tx evaluate --tx tx.unsigned    → get ExUnits
11. utxray tx build --spec tx.json \       → final transaction (with ExUnits)
         --exec-units eval-result.json
12. utxray tx sign --signing-key me.skey   → sign
13. utxray tx submit --tx tx.signed        → submit to testnet
```

Or use the high-level orchestrator:

```bash
utxray auto --validator escrow.spend --purpose spend --scenario full \
            --datum '...' --redeemer '...' --tx-spec tx.json
```

---

## Architecture

```
utxray/
├── crates/
│   ├── utxray-cli/          # Thin CLI shell (clap routing + output formatting)
│   └── utxray-core/         # All business logic
│       ├── aiken/            # Aiken CLI subprocess wrapper
│       ├── backend/          # Blockfrost HTTP client
│       ├── cbor/             # CBOR decode/encode/diff/schema (pallas)
│       ├── tx/               # Transaction build/sign/submit/simulate
│       ├── diagnose/         # Error classification rules engine
│       ├── replay/           # Bundle/run/diff
│       └── ...
├── tests/                    # Integration tests + fixtures
├── validation/v2/            # E2E validation pipeline
└── docs/                     # Spec, scaffold, references
```

**Key dependencies:**

| Crate | Purpose |
|-------|---------|
| `pallas-primitives` | Cardano Conway-era types (Tx, PlutusData, Redeemer) |
| `pallas-codec` | CBOR encode/decode via minicbor |
| `pallas-crypto` | Blake2b hashing |
| `pallas-addresses` | Bech32 address parsing |
| `reqwest` (rustls) | HTTP client for Blockfrost |
| `ed25519-dalek` | Transaction signing |
| `clap` (derive) | CLI framework |
| `serde` / `serde_json` | JSON serialization |
| `tokio` | Async runtime |

---

## Validation

The project includes an automated E2E validation pipeline that covers all 34 active commands across three tiers:

```bash
# Local commands only (no Blockfrost needed)
bash validation/v2/bin/run_v2.sh --mode local

# Full validation including chain commands
BLOCKFROST_PROJECT_ID=previewXXX bash validation/v2/bin/run_v2.sh --mode full
```

**Latest results: 36/36 PASS** — including real transaction submit + post-submit UTxO verification on Preview testnet.

<details>
<summary>Validation tiers</summary>

| Tier | Cases | What it tests |
|------|-------|---------------|
| `local-real` | 24 | All offline commands with real Aiken projects |
| `live-read` | 4 | UTxO query, datum resolve, context params/tip via Blockfrost |
| `live-write` | 8 | tx build → evaluate → sign → submit + post-submit verify |

</details>

---

## Development

```bash
# Quality gates (all must pass)
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
cargo test --workspace

# Extended verification
bash scripts/verify.sh 1
```

**Constraints enforced:**

- Zero `unwrap()` / `panic!()` in `utxray-core/src/` (outside `#[cfg(test)]`)
- Zero `std::process::Command` — all subprocess calls use `tokio::process::Command`
- All error paths return structured JSON, never raw panic output
- `status: "error"` always exits with code 1

---

## Documentation

| Document | Purpose |
|----------|---------|
| `docs/spec.md` | Complete interface spec (35 commands, JSON contracts) |
| `docs/scaffold.md` | Rust architecture, crate choices, conventions |
| `docs/references.md` | External spec links (CDDL, Blockfrost API, CIPs) |
| `docs/command-manifest.md` | 35-command acceptance checklist |
| `docs/coverage-matrix.md` | Per-command test coverage matrix |
| `docs/verification-report.md` | Latest E2E verification evidence |

---

## License

[Apache-2.0](./LICENSE)
