# utxray — Claude Code 自驱动方案

> 不用 headless 模式，不用外层脚本，不需要人工确认。
> 在 Claude Code 交互式 CLI 中一句话启动，它自己跑完全部流程。

---

## 原理

把 master agent 和 subagent 合并为一个自驱动循环：

```
Claude Code = 实现者 + 验证者 + 决策者

循环：
  1. 实现当前 phase 的下一个任务
  2. 调用 scripts/verify.sh 独立验证
  3. 读取验证结果 JSON
  4. 如果全 pass → 推进 phase
  5. 如果有 fail → 修复并重新验证
  6. 直到所有 phase 完成
```

关键：`scripts/verify.sh` 是客观裁判——Claude Code 不能绕过它声称完成，
必须拿到 `"all_pass": true` 才能推进。

---

## 文件结构

```
utxray/
├── CLAUDE.md                         # 唯一控制文件
├── scripts/
│   └── verify.sh                     # 独立验证脚本
├── docs/
│   ├── spec.md                       # 接口定义（从 artifact 复制）
│   ├── scaffold.md                   # 技术脚手架（从 artifact 复制）
│   ├── master-prompt.md              # 完整 phase gate 定义（从 artifact 复制）
│   ├── phase-state.json              # 状态文件（自动维护）
│   ├── spec-gaps.md
│   ├── deviations.md
│   ├── coverage-matrix.md
│   └── verification-report.md
├── crates/                           # 代码（自动生成）
├── tests/                            # 测试（自动生成）
└── .github/workflows/ci.yml          # CI（自动生成）
```

---

## CLAUDE.md

```markdown
# utxray — Self-Driven Development

## Identity

You are both the implementer and the verifier for the utxray project.
You work autonomously. No human approval is needed at any step.
You make implementation decisions, verify them objectively, and advance phases on your own.

## Source of truth

Read these BEFORE writing any code (first time only):
1. `docs/spec.md` — All command interfaces, JSON contracts, status rules
2. `docs/scaffold.md` — Rust workspace structure, crate choices, conventions
3. `docs/master-prompt.md` — Phase gates with exact success criteria

When any two documents conflict, spec.md wins.

## Your autonomous loop

You MUST follow this loop. Do not deviate from it.

### Step 1: Check current state
```bash
cat docs/phase-state.json
```
This tells you: current phase, attempt count, what failed last time.

### Step 2: Implement
Work on the current phase. Follow these sub-rules:
- For each command: write integration test FIRST, then implement
- Use fixtures in tests/fixtures/
- After each command is done, update docs/coverage-matrix.md
- After each functional unit (one command, one fix, one phase milestone), commit AND push:
  ```bash
  git add -A && git commit -m "<type>: <description>" && git push
  ```
  Commit message conventions:
  - `feat: implement <command>` — new command working with tests
  - `test: add fixtures for <command>` — test infrastructure
  - `fix: <what was broken>` — fixing a failing verification
  - `chore: phase N scaffold` — phase-level infrastructure
  - `docs: update coverage-matrix / verification-report` — doc updates

  Do NOT batch multiple commands into one commit. Each command = its own commit + push.

### Step 3: Verify
Run the verification script:
```bash
bash scripts/verify.sh <current_phase>
```
Then read the result:
```bash
cat docs/verification-result.json
```

### Step 4: Decide
Read the `all_pass` field in verification-result.json:

- If `true`: advance to next phase by running:
  ```bash
  cat docs/phase-state.json | jq --argjson p <next_phase> '.current_phase = $p | .attempt = 1 | .last_failure_summary = null | .phases_completed += [($p - 1)]' > /tmp/ps.json && mv /tmp/ps.json docs/phase-state.json
  ```
  Then update docs/verification-report.md with evidence.
  Then commit and push:
  ```bash
  git add -A && git commit -m "chore: phase <N> complete, advancing to phase <N+1>" && git push
  ```
  Then go back to Step 1.

- If `false`: read which checks failed, fix them, increment attempt:
  ```bash
  cat docs/phase-state.json | jq '.attempt += 1' > /tmp/ps.json && mv /tmp/ps.json docs/phase-state.json
  ```
  Then go back to Step 2.

### Step 5: Completion
When phase-state.json shows `current_phase` > 1 (for v1 core scope):
- Write final docs/verification-report.md
- Commit and push:
  ```bash
  git add -A && git commit -m "chore: utxray v1 core complete" && git push
  ```
- Say: "utxray v1 core complete. All phase gates pass. See docs/verification-report.md."
- STOP.

## Phase definitions (summary)

### Phase 0 — Scaffold
- Rust workspace (2 crates: utxray-cli + utxray-core)
- clap command stubs for all 35 commands
- Output<T> + Outcome enum + Status enum
- .utxray.toml config loading
- utxray env returns valid JSON
- .utxray.toml.example
- .github/workflows/ci.yml
- All error paths return structured JSON

### Phase 1 — Local commands
Implement with integration tests: build, typecheck, test, trace, schema validate,
cbor decode, script-data-hash, redeemer-index, tx build, diagnose, replay bundle, replay run.
See docs/master-prompt.md for exact criteria per command.

## Architecture rules

- 2-crate workspace: utxray-cli (thin shell) + utxray-core (all logic)
- Aiken: `tokio::process::Command` subprocess calls (NOT aiken-project crate)
- CBOR: pallas-codec + pallas-primitives + pallas-traverse
- Backend: concrete BlockfrostBackend struct (NO trait abstraction in v1)
- Output: Output<T> wrapper with v, status, warnings
- TLS: rustls everywhere (no native-tls)
- Errors: anyhow in CLI layer, thiserror in core layer

## Forbidden patterns (scripts/verify.sh will catch these)

- No `unwrap()` or `panic!()` in crates/utxray-core/src/ (except #[cfg(test)])
- No `std::process::Command` — use tokio::process::Command
- No `pub enum Result` — use Outcome for pass/fail
- No native-tls features in Cargo.toml

## Status rules (memorize)

- `status: "error"` = tool itself failed (bad config, network, compile error)
- `status: "ok"` or `"mixed"` = tool ran fine; check sub-item `result: "pass"|"fail"`
- `status: "error"` is NEVER used for validator-returned-False or datum-validation-failure

## Quality gates (verify.sh runs these, but you should also run them)

```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
cargo test --workspace
```

## Progress reporting

After implementing each command, state briefly:
- Command name
- Tests passing count
- Any [SPEC_GAP] found

Do NOT wait for human response between commands. Keep going.

## Max attempts

If you fail verification 5 times for the same phase, stop and report what's blocking.
Otherwise, keep looping autonomously.
```

---

## scripts/verify.sh

```bash
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
```

---

## docs/phase-state.json

```json
{
  "current_phase": 0,
  "attempt": 1,
  "phases_completed": [],
  "last_failure_summary": null
}
```

---

## 初始化与启动

### 一次性设置

```bash
mkdir utxray && cd utxray && git init

# 设置远程仓库（push 需要）
git remote add origin git@github.com:<your-org>/utxray.git
# 或 https: git remote add origin https://github.com/<your-org>/utxray.git

mkdir -p docs scripts tests/fixtures crates .github/workflows

# 复制文档内容到对应文件：
# - CLAUDE.md          ← 上面的 CLAUDE.md 内容
# - docs/spec.md       ← 接口定义 artifact
# - docs/scaffold.md   ← 技术脚手架 artifact
# - docs/master-prompt.md ← master prompt artifact
# - docs/references.md ← 权威参考源映射 artifact
# - scripts/verify.sh  ← 上面的验证脚本

# 初始化状态和追踪文件
cp 上面的 phase-state.json docs/phase-state.json

cat > docs/spec-gaps.md << 'EOF'
# Spec Gaps
No gaps recorded yet.
EOF

cat > docs/deviations.md << 'EOF'
# Approved Deviations
No deviations recorded yet.
EOF

cat > docs/coverage-matrix.md << 'EOF'
# Coverage Matrix

| Command | Implemented | Success test | Failure test | Fixture-backed | Backend | Status |
|---|---|---|---|---|---|---|
| build | no | no | no | no | none | not started |
| typecheck | no | no | no | no | none | not started |
| test | no | no | no | no | none | not started |
| trace | no | no | no | no | none | not started |
| schema validate | no | no | no | no | none | not started |
| cbor decode | no | no | no | no | none | not started |
| script-data-hash | no | no | no | no | none | not started |
| redeemer-index | no | no | no | no | none | not started |
| tx build | no | no | no | no | none | not started |
| diagnose | no | no | no | no | none | not started |
| replay bundle | no | no | no | no | none | not started |
| replay run | no | no | no | no | none | not started |
| env | no | no | no | no | all | not started |
EOF

cat > docs/verification-report.md << 'EOF'
# Verification Report
No phases completed yet.
EOF

chmod +x scripts/verify.sh

git add -A && git commit -m "chore: initial project setup with spec and process docs" && git push -u origin main
```

### 启动

```bash
claude --dangerously-skip-permissions
```

> `--dangerously-skip-permissions` 跳过所有"Do you want to proceed?"确认，
> 包括文件写入、命令执行、git 操作等。Claude Code 会自动批准一切操作。
> 仅在你信任 CLAUDE.md 中定义的操作范围时使用。

进入 Claude Code 后，输入一句话：

```
Read CLAUDE.md, then begin. Follow the autonomous loop defined there.
Start from Phase 0. Do not stop until all phases are complete or you hit max attempts.
Do not ask me any questions — make decisions yourself based on the docs.
Commit and push after every functional unit. Do not batch commits.
```

然后放手。Claude Code 会自己：
1. 读 CLAUDE.md → 读 spec.md + scaffold.md
2. 检查 phase-state.json → Phase 0
3. 实现 Phase 0 scaffold
4. 运行 `bash scripts/verify.sh 0`
5. 读 verification-result.json
6. 如果 pass → 更新 phase-state.json → 开始 Phase 1
7. 如果 fail → 修复 → 重新验证
8. 循环直到完成

---

## 为什么这比 headless 方案更好

| | Headless 方案 | 自驱动方案 |
|---|---|---|
| 复杂度 | bash 脚本编排 + prompt 拼接 | 一个 CLAUDE.md + 一个 verify.sh |
| 上下文 | 每次 headless 调用是全新会话，丢失中间状态 | 交互式会话保持完整上下文 |
| 调试 | 看日志文件 | 实时看 Claude Code 输出 |
| 可靠性 | 依赖 prompt 精确传递上次失败信息 | Claude Code 自己记得刚才做了什么 |
| 灵活性 | 改 master.sh 才能调整行为 | 随时可以插一句话修正方向 |

核心优势：**Claude Code 在交互式会话中有完整的工作记忆**——它知道自己 5 分钟前改了哪行代码、哪个测试刚刚 fail 了。headless 模式每次都是全新会话，必须从文件重建上下文。

---

## 如果想中途介入

虽然设计为全自动，但你随时可以：

```
# 暂停：直接在 Claude Code 中输入
Stop. Show me current status: cat docs/phase-state.json && cat docs/verification-result.json

# 跳过某个卡住的测试
Skip test 1.9 for now, mark it as [SPEC_GAP] in docs/spec-gaps.md, continue with next command.

# 强制推进 phase（谨慎使用）
Phase 0 is good enough. Update phase-state.json to phase 1 and continue.

# 完全自动恢复
Continue the autonomous loop from where you left off.
```

---

## 如果会话断了

重新启动 Claude Code，输入：

```bash
claude --dangerously-skip-permissions
```

然后：

```
Read CLAUDE.md. Then run: cat docs/phase-state.json && cat docs/coverage-matrix.md && git log --oneline -10
to understand where we left off. Continue the autonomous loop.
```

Claude Code 会从 phase-state.json 恢复状态，从 coverage-matrix.md 知道哪些命令已完成，从 git log 看到最近的提交，继续推进。