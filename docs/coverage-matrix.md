# Coverage Matrix

## Phase 1 — Local / offline commands

| Command | Implemented | Success test | Failure test | Fixture-backed | Backend required | Status |
|---|---|---|---|---|---|---|
| `build` | yes | yes (ignored, needs aiken) | yes | yes (hello_world) | none | complete |
| `typecheck` | yes | yes (ignored, needs aiken) | yes | yes (hello_world) | none | complete |
| `test` | yes | yes (ignored, needs aiken) | yes | yes (escrow) | none | complete |
| `trace` | yes | yes (unit tests) | yes | yes (escrow blueprint) | none | complete |
| `schema validate` | yes | yes | yes | yes (escrow blueprint) | none | complete |
| `cbor decode` | yes | yes | yes | yes (cbor fixtures) | none | complete |
| `script-data-hash` | yes | yes | yes | no (inline data) | none | complete |
| `redeemer-index` | yes | yes (unit tests) | yes | no | none | complete |
| `tx build` | yes | yes | yes | yes (tx_spec_valid) | none | complete |
| `diagnose` | yes | yes | yes | yes (test_fail_result) | none | complete |
| `replay bundle` | yes | yes | yes | yes (test_fail_result) | none | complete |
| `replay run` | yes | yes | yes | yes (bundle roundtrip) | none | complete |

## Phase 0 — Scaffold (completed)

| Command | Implemented | Success test | Failure test | Fixture-backed | Backend required | Status |
|---|---|---|---|---|---|---|
| `env` | yes | yes | yes | no | none | complete |

## Phase 2+ — Not in scope for v1 core

| Command | Implemented | Success test | Failure test | Fixture-backed | Backend required | Status |
|---|---|---|---|---|---|---|
| `tx evaluate` | no | no | no | no | blockfrost | not started |
| `tx simulate` | no | no | no | no | ogmios/local | not started |
| `tx sign` | no | no | no | no | none | not started |
| `tx submit` | no | no | no | no | blockfrost | not started |
| `utxo query` | no | no | no | no | blockfrost | not started |
| `utxo diff` | no | no | no | no | blockfrost | not started |
| `datum resolve` | no | no | no | no | blockfrost | not started |
| `context params` | no | no | no | no | ogmios/blockfrost | not started |
| `context tip` | no | no | no | no | ogmios/blockfrost | not started |
| `replay diff` | no | no | no | no | none | not started |
| `budget` | no | no | no | no | none | not started |
| `budget compare` | no | no | no | no | none | not started |
| `auto` | no | no | no | no | varies | not started |
| `gen-context` | no | no | no | no | none | not started |
| `blueprint` | no | no | no | no | none | not started |
| `blueprint apply` | no | no | no | no | none | not started |
| `blueprint convert` | no | no | no | no | none | not started |
| `uplc eval` | no | no | no | no | none | not started |
| `test-sequence` | no | no | no | no | none | not started |
| `scaffold test` | no | no | no | no | none | not started |
