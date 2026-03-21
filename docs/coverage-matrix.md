# Coverage Matrix

## Phase 1 — Local / offline commands (production-grade)

| Command | Implemented | Success test | Failure test | Fixture-backed | Backend required | Status |
|---|---|---|---|---|---|---|
| `build` | yes | yes (ignored, needs aiken) | yes | yes (hello_world) | none | complete |
| `typecheck` | yes | yes (ignored, needs aiken) | yes | yes (hello_world) | none | complete |
| `test` | yes | yes (ignored, needs aiken) | yes | yes (escrow) | none | complete |
| `trace` | yes | yes (unit+context) | yes | yes (escrow blueprint) | none | complete (Mode A context construction) |
| `schema validate` | yes | yes | yes | yes (escrow blueprint) | none | complete |
| `cbor decode` | yes | yes | yes | yes (cbor fixtures) | none | complete |
| `script-data-hash` | yes | yes | yes | no (inline data) | none | complete (real CBOR encoding) |
| `redeemer-index` | yes | yes (unit tests) | yes | no | none | complete |
| `tx build` | yes | yes | yes | yes (tx_spec_valid) | none | complete (real Conway CBOR) |
| `diagnose` | yes | yes | yes | yes (test_fail_result) | none | complete |
| `replay bundle` | yes | yes | yes | yes (test_fail_result) | none | complete |
| `replay run` | yes | yes | yes | yes (bundle roundtrip) | none | complete |

## Phase 0 — Scaffold (completed)

| Command | Implemented | Success test | Failure test | Fixture-backed | Backend required | Status |
|---|---|---|---|---|---|---|
| `env` | yes | yes | yes | no | blockfrost (optional) | complete (with Blockfrost health+tip) |

## Phase 2 — Chain-connected commands

| Command | Implemented | Success test | Failure test | Fixture-backed | Backend required | Status |
|---|---|---|---|---|---|---|
| `utxo query` | yes | yes (ignored, needs Blockfrost) | yes | yes (fixture JSON) | blockfrost | complete |
| `datum resolve` | yes | yes (ignored, needs Blockfrost) | yes | yes (fixture JSON) | blockfrost | complete |
| `tx evaluate` | yes | yes (ignored, needs Blockfrost) | yes | no | blockfrost | complete |
| `context params` | yes | yes (ignored, needs Blockfrost) | yes | yes (fixture JSON) | blockfrost | complete |
| `context tip` | yes | yes (ignored, needs Blockfrost) | yes | yes (fixture JSON) | blockfrost | complete |

## Phase 3+ — Not yet implemented

| Command | Implemented | Backend required | Status |
|---|---|---|---|
| `tx simulate` | no | ogmios/local | not started |
| `tx sign` | no | none | not started |
| `tx submit` | no | blockfrost | not started (backend method exists) |
| `utxo diff` | no | blockfrost | not started |
| `auto` | no | varies | not started |
| `gen-context` | no | none | not started |
| `replay diff` | no | none | not started |
| `budget` / `budget compare` | no | none | not started |
| `blueprint` / `apply` / `convert` | no | none | not started |
| `uplc eval` | no | none | not started |
