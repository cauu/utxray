# Approved Deviations

### DEV-001: trace constructs ScriptContext but does not execute UPLC ~~(RESOLVED: context construction)~~
- **What was changed vs spec**: trace now builds a complete Aiken-compatible ScriptContext (Mode A auto-fill) with proper transaction structure, but does not perform actual Plutus VM execution.
- **Current state**: The constructed context is included in output (`constructed_context` field). Execution requires aiken CLI with matching test infrastructure.
- **Why**: UPLC evaluation requires either embedding the Plutus VM or creating temporary aiken test files. Context construction alone is useful for AI agents to reason about validator behavior.
- **Approved by master agent**: yes

### ~~DEV-002: tx build produces JSON description, not CBOR~~ RESOLVED
- **Status**: Fixed. `tx build` now produces real Conway-era Cardano CBOR transactions using pallas-primitives types. The tx file contains hex-encoded CBOR decodable by pallas/cardano-cli.
- **Resolved in commit**: a7dd7b0

### ~~DEV-003: script-data-hash uses blake2b-256 of JSON, not CBOR~~ RESOLVED
- **Status**: Fixed. `script-data-hash` now uses proper CBOR encoding: PlutusData JSON → pallas PlutusData → minicbor CBOR bytes. Cost models encoded as canonical CBOR map with sorted keys per Alonzo spec.
- **Resolved in commit**: da9b274

### DEV-004: test-watch deferred
- **What**: `test-watch` is not implemented in v1
- **Why**: Spec marks it as deferred ("对 AI 无直接价值")
- **Status**: deferred by product decision
