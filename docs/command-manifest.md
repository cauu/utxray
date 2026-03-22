# Command Manifest — utxray v1 Production Release

> Generated from spec.md. This is the acceptance checklist for production readiness.
> 35 commands total: 34 active + 1 deferred.

## Legend

- **Status**: `done` = implemented + tested | `stub` = NOT_IMPLEMENTED | `deferred` = intentionally not in v1
- **Tests**: S = success test | F = failure test | M = malformed input test | L = live/env-gated test

## P0 — Must-have (17 commands)

| # | Command | CLI Path | Status | Tests | Notes |
|---|---|---|---|---|---|
| 1 | build | `utxray build` | done | S(ign) F | Requires aiken |
| 2 | typecheck | `utxray typecheck` | done | S(ign) F | Requires aiken |
| 3 | test | `utxray test` | done | S(ign) F | Requires aiken |
| 4 | trace | `utxray trace` | done | S F M | Mode A context construction |
| 5 | schema validate | `utxray schema validate` | done | S F M | CIP-0057 blueprint validation |
| 6 | cbor decode | `utxray cbor decode` | done | S F M | pallas CBOR decoding |
| 7 | script-data-hash | `utxray script-data-hash` | done | S F M | Real CBOR encoding |
| 8 | redeemer-index | `utxray redeemer-index` | done | S F M | Sorted input indexing |
| 9 | tx build | `utxray tx build` | done | S F M | Conway CBOR, change output, script_data_hash |
| 10 | tx evaluate | `utxray tx evaluate` | done | F L | Blockfrost proxy |
| 11 | tx simulate | `utxray tx simulate` | stub | — | **PL-04** |
| 12 | utxo query | `utxray utxo query` | done | F L | Blockfrost pagination |
| 13 | datum resolve | `utxray datum resolve` | done | F L | indexer/unresolved |
| 14 | replay bundle | `utxray replay bundle` | done | S F | Bundle creation |
| 15 | replay run | `utxray replay run` | done | S F | Bundle replay |
| 16 | diagnose | `utxray diagnose` | done | S F | Rule-based classifier |
| 17 | env | `utxray env` | done | S F | With Blockfrost health |

## P1 — Important (11 commands)

| # | Command | CLI Path | Status | Tests | Notes |
|---|---|---|---|---|---|
| 18 | auto | `utxray auto` | stub | — | **PL-12** |
| 19 | cbor diff | `utxray cbor diff` | stub | — | **PL-06** |
| 20 | context params | `utxray context params` | done | F L | Blockfrost |
| 21 | context tip | `utxray context tip` | done | F L | Blockfrost |
| 22 | tx sign | `utxray tx sign` | done | S F | ed25519-dalek |
| 23 | tx submit | `utxray tx submit` | done | F L | Blockfrost + mainnet safety |
| 24 | utxo diff | `utxray utxo diff` | stub | — | **PL-05** |
| 25 | replay diff | `utxray replay diff` | stub | — | **PL-07** |
| 26 | budget (show) | `utxray budget show` | stub | — | **PL-08** |
| 27 | budget compare | `utxray budget compare` | stub | — | **PL-09** |
| 28 | gen-context | `utxray gen-context` | stub | — | **PL-11** |

## P2 — Can defer (6 + 1 deferred)

| # | Command | CLI Path | Status | Tests | Notes |
|---|---|---|---|---|---|
| 29 | blueprint (show) | `utxray blueprint show` | stub | — | **PL-10** |
| 30 | blueprint apply | `utxray blueprint apply` | stub | — | **PL-10** |
| 31 | blueprint convert | `utxray blueprint convert` | stub | — | **PL-10** |
| 32 | uplc eval | `utxray uplc eval` | stub | — | **PL-13** |
| 33 | test-sequence | `utxray test-sequence` | stub | — | **PL-14** |
| 34 | scaffold test | `utxray scaffold test` | stub | — | **PL-15** |
| 35 | test-watch | — | deferred | — | **PL-16** (not in CLI) |

## Summary

| Category | Done | Stub | Deferred | Total |
|---|---|---|---|---|
| P0 | 16 | 1 | 0 | 17 |
| P1 | 5 | 6 | 0 | 11 |
| P2 | 0 | 6 | 1 | 7 |
| **Total** | **21** | **13** | **1** | **35** |

## Acceptance rule

Production release requires:
- All P0 commands: `done` with S+F+M tests
- All P1 commands: `done` with S+F tests
- P2 commands: `done` or explicitly `deferred` in docs/deviations.md
- Zero `stub` status remaining
