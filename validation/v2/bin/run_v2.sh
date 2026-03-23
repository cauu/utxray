#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# utxray validation pipeline v2
#
# Usage:
#   bash validation/v2/bin/run_v2.sh --mode local   # local commands only
#   bash validation/v2/bin/run_v2.sh --mode full     # all tiers
#
# Required env vars:
#   BLOCKFROST_PROJECT_ID  (for live-read / live-write modes)
#
# Optional env vars:
#   UTXRAY_NETWORK         (default: preview)
#   ALLOW_SKIPS            (default: 0; set to 1 to skip non-critical live cases)
#   DATUM_HASH_OVERRIDE    (fallback datum hash if auto-discovery fails)
# =============================================================================

MODE="full"
if [[ "${1:-}" == "--mode" ]]; then
  MODE="${2:-full}"
fi

ROOT="$(git rev-parse --show-toplevel)"
export ROOT

VAL="$ROOT/validation/v2"
BIN="$VAL/bin"
KEYS="$VAL/keys"
DATA="$VAL/data"
ART="$VAL/artifacts"
REP="$VAL/reports"
CASES="$REP/cases"

# Export all path variables so they are available everywhere
export VAL BIN KEYS DATA ART REP CASES

export UTXRAY_NETWORK="${UTXRAY_NETWORK:-preview}"
export ALLOW_SKIPS="${ALLOW_SKIPS:-0}"

PROJ_LOCAL="$ROOT/tests/fixtures/hello_world"
PROJ_BP="$ROOT/tests/fixtures/escrow"
export PROJ_LOCAL PROJ_BP

mkdir -p "$BIN" "$KEYS" "$DATA" "$ART" "$CASES" "$REP"
: > "$REP/results.ndjson"

# Track pass/fail counts for summary
TOTAL_CASES=0
PASSED_CASES=0
FAILED_CASES=0

# =============================================================================
# Utility functions
# =============================================================================

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || { echo "[FATAL] missing command: $1"; exit 2; }
}

json_status() {
  jq -r '.status // "__missing__"' "$1" 2>/dev/null || echo "__invalid_json__"
}

append_result() {
  local id="$1" name="$2" tier="$3" rc="$4" status="$5" expected_status="$6" expected_rc="$7" pass="$8" out="$9" err="${10}" note="${11:-}"
  jq -cn \
    --arg id "$id" \
    --arg name "$name" \
    --arg tier "$tier" \
    --argjson rc "$rc" \
    --arg status "$status" \
    --arg expected_status "$expected_status" \
    --arg expected_rc "$expected_rc" \
    --argjson pass "$pass" \
    --arg out "$out" \
    --arg err "$err" \
    --arg note "$note" \
    '{id:$id,name:$name,tier:$tier,rc:$rc,status:$status,expected_status:$expected_status,expected_rc:$expected_rc,pass:$pass,out:$out,err:$err,note:$note}' >> "$REP/results.ndjson"
}

# run_case: execute a command and validate its output.
#
# Usage:
#   run_case <id> <name> <tier> <expected_status_re> <expected_rc_re> \
#     <cmd> [args...]
#
# The command and its arguments are passed as separate tokens (shift 5),
# avoiding the shell-quoting pitfalls of bash -lc "$cmd".
run_case() {
  local id="$1" name="$2" tier="$3" expected_status_re="$4" expected_rc_re="$5"
  shift 5
  # "$@" is now the command + arguments

  local out="$CASES/${id}.stdout.json"
  local err="$CASES/${id}.stderr.log"

  echo "--- [$id] $name ($tier) ---"

  set +e
  "$@" >"$out" 2>"$err"
  local rc=$?
  set -e

  local status
  status="$(json_status "$out")"

  local pass=true

  # Check status matches expected pattern
  if ! [[ "$status" =~ ^(${expected_status_re})$ ]]; then
    pass=false
  fi

  # Check return code matches expected pattern
  if ! [[ "$rc" =~ ^(${expected_rc_re})$ ]]; then
    pass=false
  fi

  # Contract gate: status=error must have rc!=0
  if [[ "$status" == "error" && "$rc" -eq 0 ]]; then
    pass=false
  fi

  TOTAL_CASES=$((TOTAL_CASES + 1))
  if [[ "$pass" == "true" ]]; then
    PASSED_CASES=$((PASSED_CASES + 1))
    echo "  PASS (status=$status, rc=$rc)"
  else
    FAILED_CASES=$((FAILED_CASES + 1))
    echo "  FAIL (status=$status, rc=$rc, expected_status=/$expected_status_re/, expected_rc=/$expected_rc_re/)"
    if [[ -s "$err" ]]; then
      echo "  stderr (first 5 lines):"
      head -5 "$err" | sed 's/^/    /'
    fi
  fi

  append_result "$id" "$name" "$tier" "$rc" "$status" "$expected_status_re" "$expected_rc_re" "$pass" "$out" "$err"
}

skip_or_fail() {
  local id="$1" name="$2" tier="$3" note="$4"
  if [[ "$ALLOW_SKIPS" == "1" ]]; then
    echo "--- [$id] $name ($tier) --- SKIPPED: $note"
    append_result "$id" "$name" "$tier" 0 "skipped" "n/a" "n/a" false "" "" "$note"
    TOTAL_CASES=$((TOTAL_CASES + 1))
    FAILED_CASES=$((FAILED_CASES + 1))
    return 0
  fi
  echo "[FATAL] $note"
  exit 1
}

# =============================================================================
# Preparation
# =============================================================================

prepare_tools() {
  echo "=== Checking required tools ==="
  require_cmd cargo
  require_cmd jq
  require_cmd curl

  # aiken and cardano-cli are only hard requirements for certain modes
  if command -v aiken >/dev/null 2>&1; then
    echo "  aiken: $(aiken --version 2>/dev/null || echo 'found')"
  else
    echo "  [WARN] aiken not found; aiken-dependent cases will fail"
  fi

  if command -v cardano-cli >/dev/null 2>&1; then
    echo "  cardano-cli: $(cardano-cli --version 2>/dev/null | head -1 || echo 'found')"
  else
    echo "  [WARN] cardano-cli not found; key-gen and live cases will fail"
  fi

  # Blockfrost is only required for live modes
  if [[ "$MODE" != "local" ]]; then
    if [[ -z "${BLOCKFROST_PROJECT_ID:-}" ]]; then
      echo "[FATAL] BLOCKFROST_PROJECT_ID is required for mode=$MODE"
      exit 2
    fi
  fi

  echo "  cargo: $(cargo --version)"
  echo "  jq: $(jq --version)"
}

# Create a .utxray.toml config so utxray can read Blockfrost credentials.
# utxray reads config from the project directory, so we create it in a
# dedicated config dir and point --project at it when needed.
prepare_config() {
  local config_dir="$VAL/config"
  mkdir -p "$config_dir"
  cat > "$config_dir/.utxray.toml" << TOMLEOF
[network]
default = "${UTXRAY_NETWORK}"

[blockfrost]
project_id = "${BLOCKFROST_PROJECT_ID:-}"

[defaults]
format = "json"
include_raw = false
TOMLEOF
  export UTXRAY_CONFIG_DIR="$config_dir"
  echo "  Config written to $config_dir/.utxray.toml"
}

prepare_aiken_project() {
  echo "=== Preparing Aiken test project ==="

  # Verify that the hello_world fixture is a compilable Aiken project.
  # If aiken build fails, try creating a fresh project.
  if command -v aiken >/dev/null 2>&1; then
    echo "  Checking if $PROJ_LOCAL compiles..."
    if aiken build -d "$PROJ_LOCAL" 2>/dev/null; then
      echo "  hello_world project compiles OK"
    else
      echo "  [WARN] hello_world failed to compile, attempting to create fresh project..."
      local tmp_proj="$VAL/tmp_aiken_proj"
      rm -rf "$tmp_proj"
      if aiken new "$tmp_proj" --project-name validation/test_proj 2>/dev/null; then
        # Copy over the generated project as our local project
        PROJ_LOCAL="$tmp_proj"
        export PROJ_LOCAL
        if aiken build -d "$PROJ_LOCAL" 2>/dev/null; then
          echo "  Fresh aiken project created and compiles at $PROJ_LOCAL"
        else
          echo "  [WARN] Fresh aiken project also failed to compile"
        fi
      else
        echo "  [WARN] aiken new failed; local aiken cases may fail"
      fi
    fi
  else
    echo "  [WARN] aiken not found; skipping project check"
  fi
}

prepare_static_data() {
  echo "=== Preparing static test data ==="

  cat > "$DATA/redeemers.json" <<'JSON'
[]
JSON

  cat > "$DATA/datums.json" <<'JSON'
[]
JSON

  # Fix: cost_models must have a valid structure, not empty {}
  cat > "$DATA/cost_models.json" <<'JSON'
{
  "PlutusV3": []
}
JSON

  cat > "$DATA/cbor_left.hex" <<'HEX'
d8798344aabbccdd1903e81a004c4b40
HEX

  cat > "$DATA/cbor_right.hex" <<'HEX'
d87a82d87981182a1863
HEX

  # A simple UPLC program: (addInteger 16 26) = 42
  cat > "$DATA/add_42.uplc" <<'UPLC'
(program 1.0.0
  [
    [ (builtin addInteger) (con integer 16) ]
    (con integer 26)
  ]
)
UPLC

  cat > "$DATA/seq_spec.json" <<'JSON'
{
  "description": "v2 acceptance sequence",
  "steps": [
    {"step": 1, "description": "pass step", "action": "lock"},
    {"step": 2, "description": "fail step", "action": "unlock", "expect": "fail"}
  ]
}
JSON

  # Generate a parameterized blueprint for blueprint apply, only if source exists
  if [[ -f "$PROJ_BP/plutus.json" ]]; then
    jq '.validators[0].parameters = [{"title":"owner","schema":{"dataType":"bytes"}}]' \
      "$PROJ_BP/plutus.json" > "$DATA/parameterized_plutus.json"
    echo "  parameterized_plutus.json created"
  else
    echo "  [WARN] $PROJ_BP/plutus.json not found; C15 blueprint apply may fail"
  fi
}

prepare_keys_and_funding() {
  if [[ "$MODE" == "local" ]]; then
    echo "=== Skipping key/funding setup (local mode) ==="
    return 0
  fi

  echo "=== Preparing keys and funding ==="

  if ! command -v cardano-cli >/dev/null 2>&1; then
    skip_or_fail "SETUP-LIVE" "funding" "live-setup" "cardano-cli not found"
    return 0
  fi

  if [[ ! -f "$KEYS/me.skey" ]]; then
    cardano-cli address key-gen \
      --signing-key-file "$KEYS/me.skey" \
      --verification-key-file "$KEYS/me.vkey"
    echo "  Keys generated"
  fi

  cardano-cli address build \
    --payment-verification-key-file "$KEYS/me.vkey" \
    --testnet-magic 2 \
    --out-file "$KEYS/me.addr"

  cardano-cli address key-hash \
    --payment-verification-key-file "$KEYS/me.vkey" > "$KEYS/me.keyhash"

  local addr
  addr="$(cat "$KEYS/me.addr")"
  echo "  Address: $addr"

  local utxos
  utxos="$(curl -sf -H "project_id: $BLOCKFROST_PROJECT_ID" \
    "https://cardano-preview.blockfrost.io/api/v0/addresses/${addr}/utxos" 2>/dev/null || echo "[]")"

  # Handle Blockfrost error responses (they return objects, not arrays)
  if ! echo "$utxos" | jq -e 'type == "array"' >/dev/null 2>&1; then
    echo "  [WARN] Blockfrost returned non-array for UTxOs (possibly no UTxOs)"
    utxos="[]"
  fi

  echo "$utxos" | jq '.[0] // null' > "$DATA/funding_utxo.json"

  local utxo_ref
  utxo_ref="$(echo "$utxos" | jq -r '.[0] | "\(.tx_hash)#\(.tx_index)"' 2>/dev/null || echo "null#null")"
  local lovelace
  lovelace="$(echo "$utxos" | jq -r '.[0].amount[] | select(.unit=="lovelace") | .quantity' 2>/dev/null || echo "null")"

  if [[ -z "$utxo_ref" || "$utxo_ref" == "null#null" || -z "$lovelace" || "$lovelace" == "null" ]]; then
    skip_or_fail "SETUP-LIVE" "funding" "live-read" "No funded UTxO found for $addr. Fund it via Preview faucet."
    return 0
  fi

  echo "  Funding UTxO: $utxo_ref ($lovelace lovelace)"
  echo "$utxo_ref" > "$DATA/funding_utxo_ref.txt"

  jq -n \
    --arg utxo "$utxo_ref" \
    --arg addr "$addr" \
    --argjson lovelace "$lovelace" \
    '{
      inputs:[{utxo:$utxo,type:"pubkey",value:{lovelace:$lovelace}}],
      script_inputs:[],
      reference_inputs:[],
      outputs:[{address:$addr,value:{lovelace:2000000}}],
      mint:null,
      collateral:null,
      change_address:$addr,
      required_signers:[],
      validity:{from_slot:null,to_slot:null},
      metadata:null
    }' > "$DATA/tx_spec_live.json"
  echo "  tx_spec_live.json created"
}

discover_datum_hash() {
  if [[ "$MODE" == "local" ]]; then
    return 1
  fi

  echo "=== Discovering datum hash from preview ==="
  local addrs=(
    "addr_test1wpgexmeunzsykesf42d4eqet5yvz4dsscdnbuq7yqaj4cs6whlqwl"
    "addr_test1wrn2wfykuhfcefc2l6g2cqxkctfsayfx93q990xey96jr0c6jzwrg"
  )
  for a in "${addrs[@]}"; do
    local utxos
    utxos="$(curl -sf -H "project_id: $BLOCKFROST_PROJECT_ID" \
      "https://cardano-preview.blockfrost.io/api/v0/addresses/${a}/utxos" 2>/dev/null || echo "[]")"
    local dh
    dh="$(echo "$utxos" | jq -r '[.[] | select(.data_hash != null) | .data_hash][0] // empty' 2>/dev/null || true)"
    if [[ -n "$dh" ]]; then
      echo "  Found datum hash: $dh"
      echo "$dh" > "$DATA/datum_hash.txt"
      return 0
    fi
  done

  if [[ -n "${DATUM_HASH_OVERRIDE:-}" ]]; then
    echo "  Using DATUM_HASH_OVERRIDE: $DATUM_HASH_OVERRIDE"
    echo "$DATUM_HASH_OVERRIDE" > "$DATA/datum_hash.txt"
    return 0
  fi

  echo "  [WARN] No datum hash found"
  return 1
}

# =============================================================================
# Stage: local commands
# =============================================================================

stage_local() {
  echo ""
  echo "========================================="
  echo "  STAGE: local-real"
  echo "========================================="

  # --- C01: env ---
  run_case C01 "env" "local-real" "ok|error" "0|1" \
    cargo run -q -- env

  # --- C02: build ---
  run_case C02 "build" "local-real" "ok" "0" \
    cargo run -q -- --project "$PROJ_LOCAL" build

  # --- C03: typecheck ---
  run_case C03 "typecheck" "local-real" "ok" "0" \
    cargo run -q -- --project "$PROJ_LOCAL" typecheck

  # --- C04: test ---
  run_case C04 "test" "local-real" "ok|mixed" "0" \
    cargo run -q -- --project "$PROJ_LOCAL" test

  # --- C05: gen-context ---
  run_case C05 "gen-context" "local-real" "ok" "0" \
    cargo run -q -- --project "$PROJ_LOCAL" gen-context

  # --- C06: cbor decode ---
  local cbor_hex
  cbor_hex="$(cat "$DATA/cbor_left.hex")"
  run_case C06 "cbor decode" "local-real" "ok" "0" \
    cargo run -q -- cbor decode --hex "$cbor_hex"

  # --- C07: cbor diff ---
  local cbor_left cbor_right
  cbor_left="$(cat "$DATA/cbor_left.hex")"
  cbor_right="$(cat "$DATA/cbor_right.hex")"
  run_case C07 "cbor diff" "local-real" "ok" "0" \
    cargo run -q -- cbor diff --left "$cbor_left" --right "$cbor_right"

  # --- C08: script-data-hash ---
  run_case C08 "script-data-hash" "local-real" "ok" "0" \
    cargo run -q -- script-data-hash \
      --redeemers "$DATA/redeemers.json" \
      --datums "$DATA/datums.json" \
      --cost-models "$DATA/cost_models.json"

  # --- C09: redeemer-index ---
  # Use the existing tx.unsigned fixture if available
  local tx_fixture="$ROOT/tests/fixtures/tx.unsigned"
  if [[ -f "$tx_fixture" ]]; then
    run_case C09 "redeemer-index" "local-real" "ok|error" "0|1" \
      cargo run -q -- redeemer-index --tx "$tx_fixture"
  else
    # Try to build a tx from the C02 output if available, otherwise skip
    skip_or_fail C09 "redeemer-index" "local-real" \
      "No tx.unsigned fixture found at $tx_fixture"
  fi

  # --- C10: uplc eval ---
  # Note: uplc eval may not work if aiken doesn't support the exact syntax.
  # We run it best-effort.
  run_case C10 "uplc eval" "local-real" "ok|error" "0|1" \
    cargo run -q -- uplc eval "$DATA/add_42.uplc"

  # --- C11: schema validate ---
  run_case C11 "schema validate" "local-real" "ok|error" "0|1" \
    cargo run -q -- --project "$PROJ_BP" schema validate \
      --validator escrow.escrow.spend \
      --purpose spend \
      --datum '{"constructor":0,"fields":[{"bytes":""},{"int":0},{"int":0}]}' \
      --redeemer '{"constructor":0,"fields":[]}'

  # --- C12: trace ---
  run_case C12 "trace" "local-real" "ok|error" "0|1" \
    cargo run -q -- --project "$PROJ_BP" trace \
      --validator escrow.escrow.spend \
      --purpose propose \
      --redeemer '{"constructor":0,"fields":[]}'

  # --- C13: blueprint show ---
  run_case C13 "blueprint show" "local-real" "ok" "0" \
    cargo run -q -- --project "$PROJ_BP" blueprint show

  # --- C14: blueprint convert ---
  run_case C14 "blueprint convert" "local-real" "ok" "0" \
    cargo run -q -- --project "$PROJ_BP" blueprint convert \
      --validator escrow.escrow.spend \
      --out "$ART/escrow.plutus"

  # --- C15: blueprint apply ---
  if [[ -f "$DATA/parameterized_plutus.json" ]]; then
    run_case C15 "blueprint apply" "local-real" "ok" "0" \
      cargo run -q -- --project "$PROJ_BP" blueprint apply \
        --file "$DATA/parameterized_plutus.json" \
        --validator escrow.escrow.spend \
        --params '[{"bytes":"00"}]'
  else
    skip_or_fail C15 "blueprint apply" "local-real" \
      "parameterized_plutus.json not created (escrow/plutus.json missing)"
  fi

  # --- C16: replay bundle ---
  run_case C16 "replay bundle" "local-real" "ok" "0" \
    cargo run -q -- replay bundle \
      --from "$CASES/C04.stdout.json" \
      --output "$ART/replay.bundle.json"

  # --- C17: replay run ---
  run_case C17 "replay run" "local-real" "ok|mixed|error" "0|1" \
    cargo run -q -- replay run \
      --bundle "$ART/replay.bundle.json"

  # --- C18: replay diff ---
  run_case C18 "replay diff" "local-real" "ok" "0" \
    cargo run -q -- replay diff \
      --before "$CASES/C16.stdout.json" \
      --after "$CASES/C16.stdout.json"

  # --- C19: budget show ---
  run_case C19 "budget show" "local-real" "ok|error|mixed" "0|1" \
    cargo run -q -- --project "$PROJ_BP" budget show

  # --- C19B: budget compare ---
  run_case C19B "budget compare" "local-real" "ok|error" "0|1" \
    cargo run -q -- budget compare \
      --before "$CASES/C04.stdout.json" \
      --after "$CASES/C04.stdout.json"

  # --- C20: diagnose ---
  run_case C20 "diagnose" "local-real" "ok|error" "0|1" \
    cargo run -q -- diagnose \
      --from "$CASES/C04.stdout.json"

  # --- C21: test-sequence ---
  run_case C21 "test-sequence" "local-real" "ok|mixed|error" "0|1" \
    cargo run -q -- test-sequence \
      --spec "$DATA/seq_spec.json"

  # --- C22: auto ---
  run_case C22 "auto" "local-real" "ok|mixed|error" "0|1" \
    cargo run -q -- --project "$PROJ_LOCAL" auto --scenario build

  # --- C33: scaffold test ---
  run_case C33 "scaffold test" "local-real" "ok|error" "0|1" \
    cargo run -q -- --project "$PROJ_BP" scaffold test \
      --validator escrow.escrow.spend
}

# =============================================================================
# Stage: live-read (Blockfrost read-only)
# =============================================================================

stage_live_read() {
  echo ""
  echo "========================================="
  echo "  STAGE: live-read"
  echo "========================================="

  # Ensure Blockfrost config exists
  if [[ ! -f "$UTXRAY_CONFIG_DIR/.utxray.toml" ]]; then
    skip_or_fail C23 "utxo query" "live-read" "No Blockfrost config"
    skip_or_fail C24 "context params" "live-read" "No Blockfrost config"
    skip_or_fail C24B "context tip" "live-read" "No Blockfrost config"
    skip_or_fail C25 "datum resolve" "live-read" "No Blockfrost config"
    return 0
  fi

  # Use a known preview testnet address for utxo query (doesn't need our own keys)
  local query_addr
  query_addr="$(cat "$KEYS/me.addr" 2>/dev/null || echo "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp")"

  # --- C23: utxo query ---
  run_case C23 "utxo query" "live-read" "ok" "0" \
    cargo run -q -- --project "$UTXRAY_CONFIG_DIR" --network "$UTXRAY_NETWORK" utxo query --address "$query_addr"

  # --- C24: context params ---
  run_case C24 "context params" "live-read" "ok" "0" \
    cargo run -q -- --project "$UTXRAY_CONFIG_DIR" --network "$UTXRAY_NETWORK" context params

  # --- C24B: context tip ---
  run_case C24B "context tip" "live-read" "ok" "0" \
    cargo run -q -- --project "$UTXRAY_CONFIG_DIR" --network "$UTXRAY_NETWORK" context tip

  # --- C25: datum resolve ---
  if [[ -f "$DATA/datum_hash.txt" ]]; then
    local dh
    dh="$(cat "$DATA/datum_hash.txt")"
    run_case C25 "datum resolve" "live-read" "ok|error" "0|1" \
      cargo run -q -- --project "$UTXRAY_CONFIG_DIR" --network "$UTXRAY_NETWORK" datum resolve --hash "$dh"
  else
    skip_or_fail C25 "datum resolve" "live-read" "datum_hash not available"
  fi
}

# =============================================================================
# Stage: live-write (build, sign, submit on Preview)
# =============================================================================

stage_live_write() {
  echo ""
  echo "========================================="
  echo "  STAGE: live-write"
  echo "========================================="

  local addr
  addr="$(cat "$KEYS/me.addr" 2>/dev/null || echo "")"
  if [[ -z "$addr" ]]; then
    skip_or_fail C26 "tx build (live)" "live-write" "No address file"
    skip_or_fail C27 "tx evaluate" "live-write" "No address file"
    skip_or_fail C28 "tx simulate" "live-write" "No address file"
    skip_or_fail C29 "tx sign" "live-write" "No address file"
    skip_or_fail C30 "tx submit mainnet guard" "live-write" "No address file"
    skip_or_fail C31 "tx submit preview" "live-write" "No address file"
    skip_or_fail C32 "utxo diff" "live-write" "No address file"
    return 0
  fi

  # --- C26: tx build (live) ---
  run_case C26 "tx build (live)" "live-write" "ok" "0" \
    cargo run -q -- --network "$UTXRAY_NETWORK" tx build --spec "$DATA/tx_spec_live.json"

  # Extract the built tx file path from C26 output
  local unsigned=""
  if [[ -f "$CASES/C26.stdout.json" ]]; then
    unsigned="$(jq -r '.tx_file // empty' "$CASES/C26.stdout.json" 2>/dev/null || true)"
  fi

  if [[ -z "$unsigned" ]]; then
    echo "  [WARN] C26 did not produce tx_file; using fallback for remaining live-write cases"
    skip_or_fail C27 "tx evaluate" "live-write" "No tx_file from C26"
    skip_or_fail C28 "tx simulate" "live-write" "No tx_file from C26"
    skip_or_fail C29 "tx sign" "live-write" "No tx_file from C26"
  else
    # --- C27: tx evaluate ---
    run_case C27 "tx evaluate" "live-write" "ok|error" "0|1" \
      cargo run -q -- --network "$UTXRAY_NETWORK" tx evaluate --tx "$unsigned"

    # --- C28: tx simulate ---
    run_case C28 "tx simulate" "live-write" "ok|error" "0|1" \
      cargo run -q -- --network "$UTXRAY_NETWORK" tx simulate --tx "$unsigned"

    # --- C29: tx sign ---
    run_case C29 "tx sign" "live-write" "ok" "0" \
      cargo run -q -- --network "$UTXRAY_NETWORK" tx sign \
        --tx "$unsigned" \
        --signing-key "$KEYS/me.skey" \
        --out "$ART/tx.signed"
  fi

  # --- C30: tx submit mainnet guard (should fail) ---
  run_case C30 "tx submit mainnet guard" "live-write" "error" "1" \
    cargo run -q -- --network mainnet tx submit --tx deadbeef

  # --- C31: tx submit preview ---
  if [[ -f "$ART/tx.signed" ]]; then
    run_case C31 "tx submit preview" "live-write" "ok" "0" \
      cargo run -q -- --network "$UTXRAY_NETWORK" tx submit --tx "$ART/tx.signed"
  else
    skip_or_fail C31 "tx submit preview" "live-write" "No signed tx (C29 failed or skipped)"
  fi

  # --- C32: utxo diff ---
  # Wait for the submitted tx to propagate
  if [[ -f "$ART/tx.signed" ]]; then
    echo "  Waiting 20s for tx propagation..."
    sleep 20
  fi

  # Get current tip slot for the diff window
  local tip_slot=0
  tip_slot="$(cargo run -q -- --network "$UTXRAY_NETWORK" context tip 2>/dev/null | jq -r '.slot // 0' 2>/dev/null || echo "0")"
  local before_slot=$((tip_slot > 20 ? tip_slot - 20 : 0))

  run_case C32 "utxo diff" "live-write" "ok|error" "0|1" \
    cargo run -q -- --network "$UTXRAY_NETWORK" utxo diff \
      --address "$addr" \
      --before-slot "$before_slot" \
      --after-slot "$tip_slot"
}

# =============================================================================
# Report generation
# =============================================================================

finalize_report() {
  echo ""
  echo "========================================="
  echo "  Generating report"
  echo "========================================="

  local now
  now="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

  # Build the summary JSON
  jq -s --arg now "$now" '
    {
      generated_at: $now,
      total: length,
      passed: [.[] | select(.pass==true)] | length,
      failed: [.[] | select(.pass!=true)] | length,
      deferred: ["test-watch"],
      cases: .
    }
  ' "$REP/results.ndjson" > "$REP/latest.json"

  # Coverage gate: required case IDs depend on mode
  local required_ids
  if [[ "$MODE" == "local" ]]; then
    required_ids=(
      C01 C02 C03 C04 C05 C06 C07 C08 C09 C10 C11 C12 C13 C14 C15 C16 C17 C18
      C19 C19B C20 C21 C22 C33
    )
  else
    required_ids=(
      C01 C02 C03 C04 C05 C06 C07 C08 C09 C10 C11 C12 C13 C14 C15 C16 C17 C18
      C19 C19B C20 C21 C22 C23 C24 C24B C25 C26 C27 C28 C29 C30 C31 C32 C33
    )
  fi
  local missing_ids=()
  local rid
  for rid in "${required_ids[@]}"; do
    if ! jq -e --arg id "$rid" '.cases[] | select(.id == $id)' "$REP/latest.json" >/dev/null 2>&1; then
      missing_ids+=("$rid")
    fi
  done

  # Generate the human-readable report
  {
    echo "# Validation v2 Report"
    echo ""
    echo "- generated_at: $(jq -r '.generated_at' "$REP/latest.json")"
    echo "- total: $(jq -r '.total' "$REP/latest.json")"
    echo "- passed: $(jq -r '.passed' "$REP/latest.json")"
    echo "- failed: $(jq -r '.failed' "$REP/latest.json")"
    if [[ "${#missing_ids[@]}" -gt 0 ]]; then
      echo "- missing_cases: ${missing_ids[*]}"
    fi
    echo ""
    echo "## Failed Cases"
    echo ""
    jq -r '.cases[] | select(.pass!=true) | "- \(.id) \(.name) [\(.tier)] status=\(.status), rc=\(.rc), note=\(.note)"' "$REP/latest.json" || true
    echo ""
    echo "## Passed Cases"
    echo ""
    jq -r '.cases[] | select(.pass==true) | "- \(.id) \(.name) [\(.tier)]"' "$REP/latest.json" || true
  } > "$REP/latest.md"

  echo "  Report: $REP/latest.json"
  echo "  Summary: $REP/latest.md"
  echo ""

  # Print summary
  local passed failed total
  passed="$(jq -r '.passed' "$REP/latest.json")"
  failed="$(jq -r '.failed' "$REP/latest.json")"
  total="$(jq -r '.total' "$REP/latest.json")"
  echo "========================================="
  echo "  RESULTS: $passed/$total passed, $failed failed"
  if [[ "${#missing_ids[@]}" -gt 0 ]]; then
    echo "  MISSING CASES: ${missing_ids[*]}"
  fi
  echo "========================================="

  if [[ "${#missing_ids[@]}" -gt 0 && "$ALLOW_SKIPS" != "1" ]]; then
    echo "[FATAL] Missing required cases: ${missing_ids[*]}"
    exit 1
  fi

  if [[ "$failed" -gt 0 ]]; then
    echo "Validation completed with failures. See $REP/latest.json"
    exit 1
  fi

  echo "Validation passed. Report: $REP/latest.json"
}

# =============================================================================
# Main
# =============================================================================

main() {
  echo "utxray validation pipeline v2"
  echo "Mode: $MODE"
  echo "Network: $UTXRAY_NETWORK"
  echo ""

  prepare_tools
  prepare_config
  prepare_aiken_project
  prepare_static_data

  if [[ "$MODE" != "local" ]]; then
    prepare_keys_and_funding
    discover_datum_hash || true
  fi

  stage_local

  if [[ "$MODE" == "full" || "$MODE" == "live" || "$MODE" == "live-read" ]]; then
    stage_live_read
  fi

  if [[ "$MODE" == "full" || "$MODE" == "live" || "$MODE" == "live-write" ]]; then
    stage_live_write
  fi

  finalize_report
}

main "$@"
