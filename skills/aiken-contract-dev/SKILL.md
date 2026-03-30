# Aiken Smart Contract Development with utxray

> Full-lifecycle Cardano/Aiken smart contract development — from scaffold to on-chain verification.
> Uses utxray as the AI-native debugger and transaction toolkit.

---

## Prerequisites Check

**STOP. Before writing any code, you MUST collect these from the user.**

### Required: Keys & Wallets

Ask the user to provide the following. Do NOT proceed until all items are confirmed:

```
I need the following before we start:

1. **Blockfrost Project ID** (Preview testnet)
   - Sign up free at https://blockfrost.io/
   - Create a project for "Cardano Preview" network
   - Provide the project ID (starts with "preview...")

2. **Payment signing key** (.skey file)
   - A Cardano ed25519 signing key in text-envelope or raw format
   - This wallet MUST have test ADA on Preview testnet
   - Generate one if you don't have it:
     ```bash
     cardano-cli address key-gen \
       --verification-key-file payment.vkey \
       --signing-key-file payment.skey
     cardano-cli address build \
       --payment-verification-key-file payment.vkey \
       --out-file payment.addr \
       --testnet-magic 2
     ```
   - Fund it at: https://docs.cardano.org/cardano-testnets/tools/faucet/

3. **Payment address** (bech32, starts with `addr_test1...`)
   - The address derived from the signing key above

4. (Optional) **Collateral UTxO**
   - A UTxO with ~5 ADA, no tokens, at your payment address
   - If not provided, utxray will pick one automatically from your address
```

Store the collected values:

| Item | Where to store |
|------|---------------|
| Blockfrost Project ID | `.utxray.toml` → `[blockfrost] project_id` |
| Signing key file | Project root or `keys/` directory (add to `.gitignore`) |
| Payment address | Used in `--address` flags for utxo query and tx build |

**Security rules:**
- NEVER commit `.skey` files to git. Add `*.skey` and `keys/` to `.gitignore` immediately.
- NEVER log or echo the signing key contents.
- ALWAYS use Preview testnet. Never use `--allow-mainnet` unless the user explicitly requests it and confirms they understand the risk.

---

## Setup

### Step 1: Install utxray

```bash
# One-liner install (prebuilt binary or build from source)
curl -sSfL https://raw.githubusercontent.com/cauu/utxray/main/install.sh | bash
```

Or from source:

```bash
git clone https://github.com/cauu/utxray.git
cd utxray && bash install.sh
```

### Step 2: Install Aiken

```bash
curl -sSfL https://install.aiken-lang.org | bash
```

Verify both:

```bash
utxray env
```

Expected: `status: "ok"` with `aiken` and `utxray` versions present.

### Step 3: Configure the project

Create `.utxray.toml` in the Aiken project root:

```toml
[network]
default = "preview"

[backend]
primary = "blockfrost"
query = "blockfrost"

[blockfrost]
project_id = "<USER_PROVIDED_BLOCKFROST_ID>"

[agent]
context_path = ".utxray/context.json"
auto_update_context = true

[defaults]
format = "json"
include_raw = false
```

Add to `.gitignore`:

```
*.skey
keys/
.utxray/
```

### Step 4: Scaffold Aiken project (if new)

```bash
aiken new <org>/<project_name>
cd <project_name>
```

---

## Development Loop

Follow this autonomous loop for each validator/feature:

```
 1. WRITE   — Implement validator logic in Aiken
 2. BUILD   — utxray build
 3. TEST    — utxray test
 4. FIX     — If tests fail: utxray diagnose → fix → goto 2
 5. SCHEMA  — utxray schema validate (datum/redeemer encoding)
 6. TRACE   — utxray trace (probe with real inputs)
 7. TX      — utxray tx build → evaluate → sign → submit
 8. VERIFY  — utxray utxo query (confirm on-chain state)
```

### Phase 1: Build & Test (Local)

```bash
# Compile — produces plutus.json blueprint
utxray build

# Type-check only (faster iteration)
utxray typecheck

# Run all tests
utxray test

# Run specific test
utxray test --match "test_name"

# If tests fail, diagnose
utxray diagnose --from ./test-result.json
```

**Interpret results using status rules:**
- `status: "ok"` — tool worked; check sub-items for `result: "pass"|"fail"`
- `status: "mixed"` — some tests pass, some fail; inspect failing items
- `status: "error"` — tool itself failed; fix config/code before retrying

### Phase 2: Schema Validation & Tracing

```bash
# Validate datum against blueprint schema
utxray schema validate --validator <name> \
  --purpose <spend|mint|withdrawal> \
  --datum '{"fields": [...]}'

# Validate redeemer
utxray schema validate --validator <name> \
  --purpose <spend|mint|withdrawal> \
  --redeemer '{"constructor": 0, "fields": []}'

# Trace validator execution with custom inputs
utxray trace --validator <name>.spend \
  --purpose spend \
  --datum '...' \
  --redeemer '...'
```

### Phase 3: Transaction Lifecycle (On-Chain)

```bash
# 1. Query UTxOs at your address
utxray utxo query --address <payment_addr>

# 2. Build unsigned transaction from spec
utxray tx build --spec tx-spec.json

# 3. Evaluate to get ExUnits (requires Blockfrost)
utxray tx evaluate --tx tx.unsigned

# 4. Rebuild with exact ExUnits
utxray tx build --spec tx-spec.json --exec-units eval-result.json

# 5. Sign
utxray tx sign --tx tx.unsigned --signing-key <path/to/payment.skey>

# 6. Submit to Preview testnet
utxray tx submit --tx tx.signed

# 7. Verify on-chain (wait ~20s after submit)
utxray utxo query --address <script_address> --with-datum
```

### Phase 4: Diagnosis & Replay (When Things Fail)

```bash
# Diagnose any failure
utxray diagnose --from <failed_output.json>

# Bundle failure for reproducible replay
utxray replay bundle --from <failed_output.json>

# Re-run bundle after fix
utxray replay run --bundle replay-bundle.json

# Compare before/after
utxray replay diff --before old-replay.json --after new-replay.json

# Budget analysis
utxray budget show --validator <name>
utxray budget compare --before old-budget.json --after new-budget.json
```

---

## Transaction Spec Format

The `tx build --spec` command takes a JSON spec file. Example for a spend transaction:

```json
{
  "inputs": [
    { "tx_hash": "abc123...", "index": 0 }
  ],
  "script_inputs": [
    {
      "tx_hash": "def456...",
      "index": 0,
      "validator": "escrow.spend",
      "datum": { "fields": [...] },
      "redeemer": { "constructor": 0, "fields": [] }
    }
  ],
  "outputs": [
    {
      "address": "addr_test1...",
      "lovelace": 5000000
    }
  ],
  "collateral": {
    "tx_hash": "789abc...",
    "index": 0
  },
  "change_address": "addr_test1..."
}
```

For mint transactions, use `mint` instead of `script_inputs`:

```json
{
  "inputs": [...],
  "mint": {
    "validator": "token.mint",
    "redeemer": { "constructor": 0, "fields": [] },
    "assets": { "token_name_hex": 1 }
  },
  "outputs": [...],
  "collateral": {...},
  "change_address": "addr_test1..."
}
```

---

## Full Auto Mode

For end-to-end orchestration in a single command:

```bash
utxray auto --validator <name>.spend --purpose spend --scenario full \
  --datum '...' --redeemer '...' --tx-spec tx-spec.json
```

Scenarios: `build`, `test`, `trace`, `tx`, `full`

`auto` stops on first failure and runs `diagnose` automatically. Fix the issue and re-run.

---

## Common Patterns

### Pattern: Iterative Fix Loop

```
while true:
  utxray build
  if error → fix Aiken source
  utxray test
  if all pass → break
  utxray diagnose --from test-result
  apply fix based on error_code and suggested_commands
```

### Pattern: Pre-Submit Checklist

Before submitting any transaction:

1. `utxray schema validate` — datum/redeemer encoding is correct
2. `utxray tx evaluate` — ExUnits are within protocol limits
3. `utxray tx simulate` — full phase-1 + phase-2 passes
4. `utxray budget show` — resource usage is acceptable
5. Only then: `utxray tx sign` + `utxray tx submit`

### Pattern: Post-Submit Verification

```bash
# Submit
utxray tx submit --tx tx.signed
# Wait for propagation
sleep 20
# Verify the script UTxO exists with expected datum
utxray utxo query --address <script_address> --with-datum
# Optionally diff UTxOs
utxray utxo diff --address <script_address> --before-tx <prev_tx> --after-tx <submitted_tx>
```

---

## Error Code Reference

utxray `diagnose` produces structured error codes. Common ones:

| Error Code | Meaning | Typical Fix |
|-----------|---------|------------|
| `DATUM_MISMATCH` | Datum doesn't match blueprint schema | Fix datum JSON structure |
| `REDEEMER_MISMATCH` | Redeemer doesn't match expected type | Check constructor index |
| `SCRIPT_HASH_MISMATCH` | Wrong script hash in transaction | Rebuild after `utxray build` |
| `BUDGET_EXCEEDED` | ExUnits over protocol limit | Optimize validator logic |
| `MISSING_COLLATERAL` | No collateral UTxO provided | Add collateral to tx spec |
| `MISSING_SIGNER` | Required signer not in transaction | Add signer pubkey hash |
| `DEADLINE_FAIL` | Validity interval check failed | Adjust tx validity range |
| `MAINNET_SAFETY_BLOCK` | Attempted mainnet submit without flag | Use `--allow-mainnet` if intentional |

---

## Key Commands Quick Reference

| Stage | Command | What it does |
|-------|---------|-------------|
| Setup | `utxray env` | Verify toolchain |
| Build | `utxray build` | Compile Aiken project |
| Build | `utxray typecheck` | Fast type-check |
| Test | `utxray test` | Run Aiken tests |
| Debug | `utxray trace` | Trace validator with custom inputs |
| Debug | `utxray diagnose` | Classify errors into actionable codes |
| Validate | `utxray schema validate` | Check datum/redeemer against blueprint |
| CBOR | `utxray cbor decode` | Decode on-chain data |
| Tx | `utxray tx build` | Build transaction CBOR |
| Tx | `utxray tx evaluate` | Get ExUnits from evaluator |
| Tx | `utxray tx simulate` | Full simulation |
| Tx | `utxray tx sign` | Sign with .skey |
| Tx | `utxray tx submit` | Submit to testnet |
| Chain | `utxray utxo query` | Query UTxOs |
| Chain | `utxray datum resolve` | Resolve datum by hash |
| Replay | `utxray replay bundle` | Package failure |
| Replay | `utxray replay run` | Reproduce & verify fix |
| Perf | `utxray budget show` | Resource usage analysis |
| Auto | `utxray auto` | Full orchestrated workflow |
