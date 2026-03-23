#!/usr/bin/env bash
set -uo pipefail

PHASE=${1:-0}
RESULT_FILE="docs/verification-result.json"

echo "=== Verifying Phase $PHASE ==="

# ─── Cross-cutting checks ───

CHECK_OK=true
cargo check --workspace 2>/dev/null || CHECK_OK=false

CLIPPY_OK=true
cargo clippy --workspace -- -D warnings 2>/dev/null || CLIPPY_OK=false

FMT_OK=true
cargo fmt --all -- --check 2>/dev/null || FMT_OK=false

TEST_OK=true
cargo test --workspace 2>&1 > /tmp/utxray-test.log || TEST_OK=false

TEST_TOTAL=$(grep -c 'test .* \.\.\.' /tmp/utxray-test.log 2>/dev/null || echo 0)
TEST_PASSED=$(grep -c '\.\.\.\ ok' /tmp/utxray-test.log 2>/dev/null || echo 0)
TEST_IGNORED=$(grep -c '\.\.\.\ ignored' /tmp/utxray-test.log 2>/dev/null || echo 0)
TEST_FAILED=$((TEST_TOTAL - TEST_PASSED - TEST_IGNORED))

UNWRAP_COUNT=0
if [ -d "crates/utxray-core/src" ]; then
  UNWRAP_COUNT=$(grep -rn 'unwrap()\|\.expect(\|panic!' crates/utxray-core/src/ 2>/dev/null | grep -v '#\[cfg(test)\]' | grep -v '#\[test\]' | grep -v 'fn.*test' | wc -l | tr -d ' ')
fi

STD_PROC_COUNT=$(grep -rn 'std::process::Command' crates/ 2>/dev/null | wc -l | tr -d ' ')

FORBIDDEN_OK=true
[ "$UNWRAP_COUNT" -gt 0 ] || [ "$STD_PROC_COUNT" -gt 0 ] && FORBIDDEN_OK=false

# ─── Gate: zero NOT_IMPLEMENTED ───

NOT_IMPL_COUNT=$(grep -rn 'NOT_IMPLEMENTED' crates/ 2>/dev/null | grep -v test | grep -v '#\[cfg(test)\]' | wc -l | tr -d ' ')
NOT_IMPL_OK=true
[ "$NOT_IMPL_COUNT" -gt 0 ] && NOT_IMPL_OK=false

# ─── Gate: command coverage ───

MANIFEST_TOTAL=35
STUB_COUNT=$(grep -rc 'NOT_IMPLEMENTED\|not yet implemented' crates/utxray-cli/src/commands/*.rs 2>/dev/null | awk -F: '{s+=$2} END{print s+0}')
IMPL_COUNT=$((MANIFEST_TOTAL - STUB_COUNT - 1)) # -1 for deferred test-watch
COVERAGE_OK=true
[ "$STUB_COUNT" -gt 0 ] && COVERAGE_OK=false

# ─── Gate: E2E smoke tests ───

ENV_OK=true
cargo run -q -- env 2>/dev/null | jq -e '.status == "ok"' >/dev/null 2>&1 || ENV_OK=false

TX_BUILD_OK=true
if [ -f "tests/fixtures/tx_spec_valid.json" ]; then
  cargo run -q -- tx build --spec tests/fixtures/tx_spec_valid.json 2>/dev/null | jq -e '.status == "ok"' >/dev/null 2>&1 || TX_BUILD_OK=false
fi

SCHEMA_OK=true
if [ -d "tests/fixtures/escrow" ]; then
  cargo run -q -- --project tests/fixtures/escrow schema validate --validator escrow.escrow.spend --purpose spend --redeemer '{"constructor":0,"fields":[]}' 2>/dev/null | jq -e '.status == "ok"' >/dev/null 2>&1 || SCHEMA_OK=false
fi

# ─── Gate: docs exist and non-empty ───

DOCS_OK=true
DOCS_MISSING=""
for doc in docs/coverage-matrix.md docs/verification-report.md docs/spec-gaps.md docs/deviations.md docs/command-manifest.md; do
  if [ ! -s "$doc" ]; then
    DOCS_OK=false
    DOCS_MISSING="$DOCS_MISSING $doc"
  fi
done

# ─── Phase-specific ───

PHASE_CHECKS=""
PHASE_OK=true

if [ "$PHASE" -eq 0 ]; then
  # utxray env works
  if cargo run -q -- env 2>/dev/null | jq -e '.v and .status' >/dev/null 2>&1; then
    PHASE_CHECKS="$PHASE_CHECKS\"env_json\": true, "
  else
    PHASE_CHECKS="$PHASE_CHECKS\"env_json\": false, "
    PHASE_OK=false
  fi

  # help shows commands
  if cargo run -q -- --help 2>/dev/null | grep -q 'build'; then
    PHASE_CHECKS="$PHASE_CHECKS\"help_commands\": true, "
  else
    PHASE_CHECKS="$PHASE_CHECKS\"help_commands\": false, "
    PHASE_OK=false
  fi

  # config example
  if [ -f ".utxray.toml.example" ]; then
    PHASE_CHECKS="$PHASE_CHECKS\"config_example\": true, "
  else
    PHASE_CHECKS="$PHASE_CHECKS\"config_example\": false, "
    PHASE_OK=false
  fi

  # CI file
  if [ -f ".github/workflows/ci.yml" ]; then
    PHASE_CHECKS="$PHASE_CHECKS\"ci_config\": true"
  else
    PHASE_CHECKS="$PHASE_CHECKS\"ci_config\": false"
    PHASE_OK=false
  fi
fi

if [ "$PHASE" -eq 1 ]; then
  # Fixtures exist
  if [ -d "tests/fixtures" ] && [ "$(ls tests/fixtures/ 2>/dev/null | wc -l)" -gt 0 ]; then
    PHASE_CHECKS="$PHASE_CHECKS\"fixtures_exist\": true, "
  else
    PHASE_CHECKS="$PHASE_CHECKS\"fixtures_exist\": false, "
    PHASE_OK=false
  fi

  # Coverage matrix updated
  if grep -q '| yes' docs/coverage-matrix.md 2>/dev/null; then
    PHASE_CHECKS="$PHASE_CHECKS\"coverage_updated\": true, "
  else
    PHASE_CHECKS="$PHASE_CHECKS\"coverage_updated\": false, "
    PHASE_OK=false
  fi

  # Verification report updated
  if grep -q 'Phase 1' docs/verification-report.md 2>/dev/null; then
    PHASE_CHECKS="$PHASE_CHECKS\"report_updated\": true, "
  else
    PHASE_CHECKS="$PHASE_CHECKS\"report_updated\": false, "
    PHASE_OK=false
  fi

  # Minimum test count (12 commands x at least 1 test each)
  if [ "$TEST_PASSED" -ge 12 ]; then
    PHASE_CHECKS="$PHASE_CHECKS\"min_test_count\": true"
  else
    PHASE_CHECKS="$PHASE_CHECKS\"min_test_count\": false"
    PHASE_OK=false
  fi
fi

if [ "$PHASE" -ge 2 ]; then
  # Chain commands have tests
  CHAIN_TEST_COUNT=$(ls crates/utxray-cli/tests/cli_utxo_diff_test.rs crates/utxray-cli/tests/cli_tx_simulate_test.rs 2>/dev/null | wc -l | tr -d ' ')
  if [ "$CHAIN_TEST_COUNT" -ge 2 ]; then
    PHASE_CHECKS="$PHASE_CHECKS\"chain_tests_exist\": true, "
  else
    PHASE_CHECKS="$PHASE_CHECKS\"chain_tests_exist\": false, "
    PHASE_OK=false
  fi

  # All docs present
  if [ "$DOCS_OK" = "true" ]; then
    PHASE_CHECKS="$PHASE_CHECKS\"docs_complete\": true, "
  else
    PHASE_CHECKS="$PHASE_CHECKS\"docs_complete\": false, "
    PHASE_OK=false
  fi

  # Zero NOT_IMPLEMENTED
  if [ "$NOT_IMPL_OK" = "true" ]; then
    PHASE_CHECKS="$PHASE_CHECKS\"zero_not_impl\": true, "
  else
    PHASE_CHECKS="$PHASE_CHECKS\"zero_not_impl\": false, "
    PHASE_OK=false
  fi

  # E2E: env works
  if [ "$ENV_OK" = "true" ]; then
    PHASE_CHECKS="$PHASE_CHECKS\"e2e_env\": true"
  else
    PHASE_CHECKS="$PHASE_CHECKS\"e2e_env\": false"
    PHASE_OK=false
  fi
fi

if [ "$PHASE" -ge 3 ]; then
  # Full command coverage
  if [ "$COVERAGE_OK" = "true" ]; then
    PHASE_CHECKS="$PHASE_CHECKS, \"full_coverage\": true, "
  else
    PHASE_CHECKS="$PHASE_CHECKS, \"full_coverage\": false, "
    PHASE_OK=false
  fi

  # E2E: tx build works
  if [ "$TX_BUILD_OK" = "true" ]; then
    PHASE_CHECKS="$PHASE_CHECKS\"e2e_tx_build\": true, "
  else
    PHASE_CHECKS="$PHASE_CHECKS\"e2e_tx_build\": false, "
    PHASE_OK=false
  fi

  # E2E: schema validate works
  if [ "$SCHEMA_OK" = "true" ]; then
    PHASE_CHECKS="$PHASE_CHECKS\"e2e_schema_validate\": true"
  else
    PHASE_CHECKS="$PHASE_CHECKS\"e2e_schema_validate\": false"
    PHASE_OK=false
  fi
fi

# ─── Aggregate ───

ALL_OK=true
for v in "$CHECK_OK" "$CLIPPY_OK" "$FMT_OK" "$TEST_OK" "$FORBIDDEN_OK" "$PHASE_OK"; do
  [ "$v" != "true" ] && ALL_OK=false
done

# ─── Failing tests detail ───
FAIL_DETAIL=""
if [ "$TEST_OK" != "true" ]; then
  FAIL_DETAIL=$(grep -A 3 'FAILED\|panicked\|failures' /tmp/utxray-test.log 2>/dev/null | head -30 || true)
fi

# ─── Write result ───

cat > "$RESULT_FILE" << ENDJSON
{
  "phase": $PHASE,
  "all_pass": $ALL_OK,
  "cross_cutting": {
    "cargo_check": $CHECK_OK,
    "cargo_clippy": $CLIPPY_OK,
    "cargo_fmt": $FMT_OK,
    "cargo_test": $TEST_OK,
    "no_forbidden_patterns": $FORBIDDEN_OK,
    "unwrap_count": $UNWRAP_COUNT,
    "std_process_count": $STD_PROC_COUNT
  },
  "v2_gates": {
    "not_implemented_count": $NOT_IMPL_COUNT,
    "zero_not_impl": $NOT_IMPL_OK,
    "manifest_total": $MANIFEST_TOTAL,
    "stub_count": $STUB_COUNT,
    "impl_count": $IMPL_COUNT,
    "coverage_ok": $COVERAGE_OK,
    "e2e_env": $ENV_OK,
    "e2e_tx_build": $TX_BUILD_OK,
    "e2e_schema_validate": $SCHEMA_OK,
    "docs_present": $DOCS_OK,
    "docs_missing": "$(echo $DOCS_MISSING | xargs)"
  },
  "test_summary": {
    "total": $TEST_TOTAL,
    "passed": $TEST_PASSED,
    "ignored": $TEST_IGNORED,
    "failed": $TEST_FAILED
  },
  "phase_checks": { $PHASE_CHECKS },
  "failing_tests": $(echo "$FAIL_DETAIL" | jq -Rs . 2>/dev/null || echo '""')
}
ENDJSON

echo ""
cat "$RESULT_FILE"
