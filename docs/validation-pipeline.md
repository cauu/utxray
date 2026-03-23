# utxray 验收流水线 v2（可执行设计）

> 目标：让 AI agent 在**真实环境**下，自动完成 utxray 全命令验收，并产出机器可读报告。
> 
> 本文档替代原“环境搭建手册”模式，改为“一键流水线 + 强门禁 + 全覆盖矩阵”。

---

## 1. 验收目标

### 1.1 范围

- 覆盖 `docs/command-manifest.md` 中全部 **34 个 active 命令**。
- `test-watch` 为产品明确 deferred（DEV-004），不纳入 CLI 执行覆盖，但需在报告中标注。

### 1.2 通过标准

流水线通过需要同时满足：

1. **覆盖门禁**：34 个 active 命令均至少执行 1 个验收用例。
2. **契约门禁**：每个用例都满足：
   - 输出可解析 JSON
   - 顶层包含 `v`、`status`
   - `status=error` 时进程退出码必须为非 0
3. **功能门禁（关键路径）**：关键用例（见 4.2）必须达到预期状态。
4. **产物门禁**：生成 `validation/v2/reports/latest.json` 和 `latest.md`。

---

## 2. 真实环境定义

v2 将“真实环境”拆成三层：

- `local-real`：真实机器、真实二进制、真实 Aiken 项目（非 mock）。
- `live-read`：接入 Preview + Blockfrost，执行链上只读命令。
- `live-write`：在 Preview 发起真实交易（build/sign/submit）并回查状态。

默认模式为 `full`：三层全部执行。

---

## 3. 前置条件

## 3.1 工具

- `cargo`
- `aiken`
- `cardano-cli`
- `jq`
- `curl`

## 3.2 环境变量

- `BLOCKFROST_PROJECT_ID`（必填，Preview）
- `UTXRAY_NETWORK`（默认 `preview`）
- `ALLOW_SKIPS`（默认 `0`，表示严格模式；`1` 允许跳过非关键 live case）

---

## 4. 命令覆盖矩阵（34 active）

下表是 v2 的最小覆盖映射（每个命令至少一个 case）。

| 命令 | case_id |
|---|---|
| `env` | `C01` |
| `build` | `C02` |
| `typecheck` | `C03` |
| `test` | `C04` |
| `trace` | `C12` |
| `schema validate` | `C11` |
| `cbor decode` | `C06` |
| `script-data-hash` | `C08` |
| `redeemer-index` | `C09` |
| `tx build` | `C26` |
| `tx evaluate` | `C27` |
| `tx simulate` | `C28` |
| `utxo query` | `C23` |
| `datum resolve` | `C25` |
| `replay bundle` | `C16` |
| `replay run` | `C17` |
| `diagnose` | `C20` |
| `auto` | `C22` |
| `cbor diff` | `C07` |
| `context params` | `C24` |
| `context tip` | `C24B` |
| `tx sign` | `C29` |
| `tx submit` | `C30` + `C31` |
| `utxo diff` | `C32` |
| `replay diff` | `C18` |
| `budget show` | `C19` |
| `budget compare` | `C19B` |
| `gen-context` | `C05` |
| `blueprint show` | `C13` |
| `blueprint apply` | `C15` |
| `blueprint convert` | `C14` |
| `uplc eval` | `C10` |
| `test-sequence` | `C21` |
| `scaffold test` | `C33` |

### 4.2 关键功能门禁（必须成功）

以下 case 必须 `status in {ok,mixed}`（且退出码=0）：

- `C01` `env`
- `C02` `build`
- `C03` `typecheck`
- `C04` `test`
- `C06` `cbor decode`
- `C07` `cbor diff`
- `C08` `script-data-hash`
- `C23` `utxo query`
- `C24` `context params`
- `C24B` `context tip`
- `C26` `tx build (live spec)`
- `C29` `tx sign`
- `C31` `tx submit (preview)`
- `C32` `utxo diff`

---

## 5. 目录与产物

```text
validation/v2/
├── bin/
│   └── run_v2.sh
├── keys/
│   ├── me.skey
│   ├── me.vkey
│   ├── me.addr
│   └── me.keyhash
├── data/
│   ├── redeemers.json
│   ├── datums.json
│   ├── cost_models.json
│   ├── seq_spec.json
│   ├── add_42.uplc
│   ├── cbor_left.hex
│   ├── cbor_right.hex
│   ├── tx_spec_live.json
│   ├── funding_utxo.json
│   ├── funding_utxo_ref.txt
│   └── datum_hash.txt
├── artifacts/
│   ├── replay.bundle.json
│   ├── tx.unsigned
│   └── tx.signed
└── reports/
    ├── cases/           # 每个 case 的 stdout/stderr
    ├── results.ndjson   # 每个 case 一行 JSON
    ├── latest.json      # 汇总报告
    └── latest.md        # 人类可读摘要
```

---

## 6. 一键执行入口（run_v2.sh）

The actual runnable script lives at `validation/v2/bin/run_v2.sh`.

```bash
bash validation/v2/bin/run_v2.sh --mode local   # local commands only
bash validation/v2/bin/run_v2.sh --mode full     # all tiers (needs BLOCKFROST_PROJECT_ID)
```

> The script defaults to strict mode (`ALLOW_SKIPS=0`). Set `ALLOW_SKIPS=1` to skip non-critical live cases.

### Key design decisions (v2 fixes over the inline draft)

1. **`run_case` uses direct execution (`”$@”`)** instead of `bash -lc “$cmd”`, avoiding shell variable expansion and quoting issues.
2. **`.utxray.toml` is created from env vars** in `prepare_config()`, so utxray can read Blockfrost credentials.
3. **Aiken project compilation is verified** in `prepare_aiken_project()`; if the fixture fails, a fresh project is created via `aiken new`.
4. **C09 redeemer-index uses the existing `tests/fixtures/tx.unsigned`** fixture (which contains valid CBOR), with a graceful skip if missing.
5. **C08 cost_models uses `{“PlutusV3”: []}**` instead of empty `{}`.
6. **C10 uplc eval is best-effort** (accepts `ok|error` status).
7. **All path variables are exported** for visibility in subprocesses.
8. **Live stages are skipped in `--mode local`**, avoiding the need for Blockfrost or cardano-cli.
9. **Graceful degradation** throughout: missing tools or failed upstream cases result in skips rather than hard aborts (when `ALLOW_SKIPS=1`).

---

## 7. 推荐 CI 接入

### 7.1 本地契约门禁（PR 必跑）

```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
cargo test --workspace
bash scripts/verify.sh 3
bash validation/v2/bin/run_v2.sh --mode local
```

### 7.2 夜间真实环境门禁（Scheduled）

```bash
bash validation/v2/bin/run_v2.sh --mode full
```

并上传：

- `validation/v2/reports/latest.json`
- `validation/v2/reports/latest.md`
- `validation/v2/reports/cases/*`

---

## 8. 常见失败与处置

- `BLOCKFROST_PROJECT_ID` 缺失：立即 fail。
- 资金不足：立即 fail（严格模式）。
- `datum_hash` 自动发现失败：
  - 严格模式：fail
  - `ALLOW_SKIPS=1`：标记 skipped 并继续
- `tx submit preview` 失败：保留 case 产物并 fail（这是关键功能门禁）。

---

## 9. 与 v1 的关键差异

- v1 是“环境搭建 + 手工 checklist”；v2 是“自动执行 + 自动判定 + 自动报告”。
- v2 显式覆盖 34 active 命令，不再只覆盖子集。
- v2 增加 `status/exit code` 一致性门禁，面向 AI agent 编排稳定性。
