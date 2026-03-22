# Coverage Matrix

## Phase 0 — Scaffold

| Command | Implemented | Success test | Failure test | Malformed test | Fixture-backed | Backend required | Status |
|---|---|---|---|---|---|---|---|
| `env` | yes | yes | yes | n/a | no | blockfrost (optional) | complete |

## Phase 1 — Local / offline commands

| Command | Implemented | Success test | Failure test | Malformed test | Fixture-backed | Backend required | Status |
|---|---|---|---|---|---|---|---|
| `build` | yes | yes (ignored, needs aiken) | yes | n/a | yes (hello_world) | none | complete |
| `typecheck` | yes | yes (ignored, needs aiken) | yes | n/a | yes (hello_world) | none | complete |
| `test` | yes | yes (ignored, needs aiken) | yes | yes | yes (escrow) | none | complete |
| `trace` | yes | yes (unit+context) | yes | yes | yes (escrow blueprint) | none | complete |
| `schema validate` | yes | yes | yes | yes | yes (escrow blueprint) | none | complete |
| `cbor decode` | yes | yes | yes | yes | yes (cbor fixtures) | none | complete |
| `script-data-hash` | yes | yes | yes | yes | no (inline data) | none | complete |
| `redeemer-index` | yes | yes (unit tests) | yes | yes | no | none | complete |
| `tx build` | yes | yes | yes | yes | yes (tx_spec_valid) | none | complete |
| `diagnose` | yes | yes | yes | n/a | yes (test_fail_result) | none | complete |
| `replay bundle` | yes | yes | yes | n/a | yes (test_fail_result) | none | complete |
| `replay run` | yes | yes | yes | n/a | yes (bundle roundtrip) | none | complete |

## Phase 2 — Chain-connected commands

| Command | Implemented | Success test | Failure test | Malformed test | Fixture-backed | Backend required | Status |
|---|---|---|---|---|---|---|---|
| `utxo query` | yes | yes (ignored, needs Blockfrost) | yes | n/a | yes (fixture JSON) | blockfrost | complete |
| `datum resolve` | yes | yes (ignored, needs Blockfrost) | yes | n/a | yes (fixture JSON) | blockfrost | complete |
| `tx evaluate` | yes | yes (ignored, needs Blockfrost) | yes | n/a | no | blockfrost | complete |
| `tx simulate` | yes | n/a | yes | yes | no | ogmios | complete |
| `context params` | yes | yes (ignored, needs Blockfrost) | yes | n/a | yes (fixture JSON) | blockfrost | complete |
| `context tip` | yes | yes (ignored, needs Blockfrost) | yes | n/a | yes (fixture JSON) | blockfrost | complete |
| `tx sign` | yes | yes | yes | n/a | yes | none | complete |
| `tx submit` | yes | n/a | yes | n/a | no | blockfrost | complete |
| `utxo diff` | yes | n/a | yes | n/a | no | blockfrost | complete |

## Phase 3 — Extended commands

| Command | Implemented | Success test | Failure test | Malformed test | Fixture-backed | Backend required | Status |
|---|---|---|---|---|---|---|---|
| `auto` | yes | yes | yes | yes | yes (escrow) | varies | complete |
| `cbor diff` | yes | yes | yes | yes | yes (cbor fixtures) | none | complete |
| `replay diff` | yes | yes | yes | yes | no | none | complete |
| `budget show` | yes | yes | yes | n/a | yes (escrow) | none | complete |
| `budget compare` | yes | n/a | yes | yes | no | none | complete |
| `gen-context` | yes | yes | yes | n/a | yes (escrow) | none | complete |
| `blueprint show` | yes | yes | yes | n/a | yes (escrow) | none | complete |
| `blueprint apply` | yes | n/a | yes | n/a | no | none | complete |
| `blueprint convert` | yes | yes | yes | n/a | yes (escrow) | none | complete |
| `uplc eval` | yes | n/a | yes | yes | no | none | complete |
| `test-sequence` | yes | yes | yes | yes | no | none | complete |
| `scaffold test` | yes | yes | yes | n/a | yes (escrow) | none | complete |

## Deferred

| Command | Status | Reason |
|---|---|---|
| `test-watch` | deferred | Not applicable for AI agents (DEV-004) |
