# Verification Report — Production Release

## Environment

- **Rust version**: `rustc 1.85.0 (4d91de4e4 2025-02-17)`
- **OS / arch**: Darwin 24.5.0 arm64
- **Date**: 2026-03-23
- **Network**: Cardano Preview Testnet

## Validation Pipeline v2 Results

See `validation/v2/reports/latest.json` for machine-readable results.

### On-chain transactions (preview testnet)
| tx_hash | purpose |
|---|---|
| `c4d1bd95019f6ae8f0440b7c131da89aefca2c837d033abee39a165c726485b4` | First E2E tx |
| `02105ad342ba2273cdad7657f9b87fac18ca1a3d689fc660be5a9b401af9ecb4` | Pipeline run 2 |
| `1822eec92372f0052bfe0adeec3206eb184ed12c147d533c17771e44ee867b80` | Pipeline run 3 |

### Post-submit verification
C32B verifies submitted tx_hash is present in UTxO set after 40s propagation.

## Quality Gates

| Gate | Result |
|---|---|
| cargo check/clippy/fmt/test | ALL PASS |
| verify.sh all_pass | true |
| NOT_IMPLEMENTED count | 0 |
| Command coverage | 34/35 (1 deferred) |

## Phase Verdicts

- Phase 0-3: all complete
- PL-00 to PL-21: all complete
- Preview testnet E2E: tx build→evaluate→sign→submit→verify

## Note

This file is updated by each validation run. For the authoritative results,
always refer to `validation/v2/reports/latest.json` and `latest.md`.
