# Spec Gaps

### GAP-001: env command output format
- **Observed issue**: The spec defines `utxray env [--check-drift <network>]` but does not include a JSON output example
- **Impacted command(s)**: env
- **Blocking**: no
- **Proposed safe default**: Output v, status, aiken info, config info, backend availability
- **Status**: resolved (implemented with reasonable defaults)

### GAP-002: trace command execution without aiken test infrastructure
- **Observed issue**: The spec describes trace as running a validator with custom inputs, but Phase 1 cannot dynamically inject datum/redeemer into aiken's test runner
- **Impacted command(s)**: trace
- **Blocking**: no (input validation works, execution requires aiken)
- **Proposed safe default**: Validate all inputs and return structured error when aiken is not available or test infrastructure doesn't support dynamic injection
- **Status**: open (execution fidelity limited in Phase 1)

### GAP-003: script-data-hash computation method
- **Observed issue**: The spec says to compute script_data_hash from redeemers, datums, and cost_models, but doesn't specify the exact serialization format for the hash input
- **Impacted command(s)**: script-data-hash
- **Blocking**: no
- **Proposed safe default**: Use canonical JSON serialization followed by blake2b-256 as a placeholder; real Cardano script_data_hash requires exact CBOR encoding per the Alonzo spec
- **Status**: open (hash may not match on-chain computation)

### GAP-004: tx build is a simplified transaction builder
- **Observed issue**: Building a real Cardano transaction requires complex CBOR construction. Phase 1 produces a JSON-based transaction description rather than actual CBOR
- **Impacted command(s)**: tx build
- **Blocking**: no for Phase 1 testing
- **Proposed safe default**: Validate tx spec, compute summary, write description file
- **Status**: open (full CBOR transaction building deferred)
