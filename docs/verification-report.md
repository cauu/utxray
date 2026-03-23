# Verification Report — Production Release

## Environment

- **Git revision**: `449363487fe5d7ff067f5d40bd0232c4e6c5fc18`
- **Rust version**: `rustc 1.85.0 (4d91de4e4 2025-02-17)`
- **OS / arch**: Darwin 24.5.0 arm64
- **Date**: 2026-03-23
- **Network**: Cardano Preview Testnet

## Validation Pipeline v2 Results

**35/35 ALL PASS** (validation/v2/reports/latest.json)

### On-chain transactions
| tx_hash | purpose |
|---|---|
| `c4d1bd95019f6ae8f0440b7c131da89aefca2c837d033abee39a165c726485b4` | First E2E tx |
| `02105ad342ba2273cdad7657f9b87fac18ca1a3d689fc660be5a9b401af9ecb4` | Pipeline re-run tx |

### Post-submit verification
Submitted tx_hash confirmed present in UTxO set after 25s propagation.

## Quality Gates

| Gate | Result |
|---|---|
| cargo check/clippy/fmt/test | ALL PASS |
| verify.sh all_pass | true |
| NOT_IMPLEMENTED count | 0 |
| Command coverage | 34/35 (1 deferred) |
| Pipeline v2 (35 cases) | 35/35 PASS |

## Phase Verdicts

- Phase 0-3: ✅ all complete
- PL-00 to PL-21: ✅ all complete
- Preview testnet E2E: ✅ tx build→evaluate→sign→submit→verify
