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
TEST_FAILED=$((TEST_TOTAL - TEST_PASSED))

UNWRAP_COUNT=0
if [ -d "crates/utxray-core/src" ]; then
  UNWRAP_COUNT=$(grep -rn 'unwrap()\|\.expect(\|panic!' crates/utxray-core/src/ 2>/dev/null | grep -v '#\[cfg(test)\]' | grep -v '#\[test\]' | grep -v 'fn.*test' | wc -l | tr -d ' ')
fi

STD_PROC_COUNT=$(grep -rn 'std::process::Command' crates/ 2>/dev/null | wc -l | tr -d ' ')

FORBIDDEN_OK=true
[ "$UNWRAP_COUNT" -gt 0 ] || [ "$STD_PROC_COUNT" -gt 0 ] && FORBIDDEN_OK=false

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

  # Minimum test count (12 commands × at least 1 test each)
  if [ "$TEST_PASSED" -ge 12 ]; then
    PHASE_CHECKS="$PHASE_CHECKS\"min_test_count\": true"
  else
    PHASE_CHECKS="$PHASE_CHECKS\"min_test_count\": false"
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
  "test_summary": {
    "total": $TEST_TOTAL,
    "passed": $TEST_PASSED,
    "failed": $TEST_FAILED
  },
  "phase_checks": { $PHASE_CHECKS },
  "failing_tests": $(echo "$FAIL_DETAIL" | jq -Rs . 2>/dev/null || echo '""')
}
ENDJSON

echo ""
cat "$RESULT_FILE"