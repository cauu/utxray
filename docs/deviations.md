# Approved Deviations

### DEV-001: trace command does not execute validators
- **What was changed vs spec**: The spec expects `trace` to actually execute a validator with the given datum/redeemer/context. The Phase 1 implementation validates all inputs, locates the validator in the blueprint, and produces structured output with `context_mode`, `cost_fidelity`, `budget_source`, and `scope` fields, but does not perform actual Plutus VM execution.
- **Why**: Aiken's test runner does not support dynamic injection of datum/redeemer from the CLI. Full execution requires either embedding the Plutus VM or a more sophisticated aiken integration.
- **Approved by master agent**: yes (Phase 1 scope)

### DEV-002: tx build produces JSON description, not CBOR
- **What was changed vs spec**: The spec expects `tx build` to produce an unsigned transaction file. The Phase 1 implementation produces a JSON transaction description file with all fields validated, fee estimated, and scripts resolved, but the output is JSON rather than Cardano-format CBOR.
- **Why**: Full CBOR transaction construction requires protocol parameters, UTxO resolution, and precise Cardano-era serialization. This is deferred to Phase 2 when chain-connected backends are available.
- **Approved by master agent**: yes (Phase 1 scope)

### DEV-003: script-data-hash uses blake2b-256 of JSON, not CBOR
- **What was changed vs spec**: The Cardano specification computes script_data_hash from CBOR-encoded redeemers, datums, and cost models. The Phase 1 implementation computes blake2b-256 of canonical JSON serialization.
- **Why**: Exact CBOR encoding for the hash input requires protocol-version-specific serialization rules. The current implementation demonstrates the hash computation pipeline and validates inputs, but the hash value will not match on-chain computation.
- **Approved by master agent**: yes (Phase 1 scope, noted in spec-gaps.md)
