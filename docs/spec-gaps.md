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
- **Status**: resolved (context construction implemented; execution deferred to UPLC eval path)

### GAP-003: script-data-hash computation method
- **Observed issue**: The spec says to compute script_data_hash from redeemers, datums, and cost_models, but doesn't specify the exact serialization format for the hash input
- **Impacted command(s)**: script-data-hash
- **Blocking**: no
- **Proposed safe default**: Use canonical JSON serialization followed by blake2b-256 as a placeholder; real Cardano script_data_hash requires exact CBOR encoding per the Alonzo spec
- **Status**: resolved (proper CBOR encoding implemented per Alonzo spec using pallas PlutusData)

### GAP-004: tx build is a simplified transaction builder
- **Observed issue**: Building a real Cardano transaction requires complex CBOR construction. Phase 1 produces a JSON-based transaction description rather than actual CBOR
- **Impacted command(s)**: tx build
- **Blocking**: no for Phase 1 testing
- **Proposed safe default**: Validate tx spec, compute summary, write description file
- **Status**: resolved (real Conway-era CBOR via pallas-primitives, change output, script_data_hash)

### GAP-005: uplc eval execution model
- **Observed issue**: The spec describes UPLC evaluation but does not specify the exact VM engine to use
- **Impacted command(s)**: uplc eval
- **Blocking**: no
- **Proposed safe default**: Parse and validate UPLC textual representation; full VM execution deferred
- **Status**: open (partial implementation, VM execution not embedded)
