# utxray CLI v4.1 (Final) — 完整接口定义文档

> Cardano / Aiken 智能合约 AI 调试工具链。
> UTxO X-Ray — 给 Cardano 合约做透视。
> 所有命令默认输出结构化 JSON（`--format json`），供 AI Agent 直接消费。

---

## 全局选项

```
utxray [--project <path>]    # 项目根目录，默认 "."
       [--network <net>]     # preview | preprod | mainnet | local，默认 "preview"
       [--format <fmt>]      # json | text，默认 json（非 TTY 自动 json）
       [--verbose]           # 附加原始工具输出
       [--include-raw]       # 在 JSON 输出中内嵌大字段（tx_cbor、raw_cbor 等），默认只输出文件路径
       [--backend <n>]       # 覆盖 .utxray.toml 中的默认 backend
```

### Script purpose

以下命令支持 `--purpose` 参数，用于指定脚本目的：

```
--purpose <spend|mint|withdrawal|certificate|propose|vote>
```

Conway 时代的 Cardano 支持 6 种 script purpose。utxray 将 purpose 作为一等公民，
而非默认 spend。`withdraw` 作为 `withdrawal` 的兼容别名接受，但规范用词统一为
`withdrawal`（与 Ogmios redeemer entity tag 一致）。

不同 purpose 影响：

- **schema validate**: mint 没有 datum；spend 的 datum 是否必需由 blueprint schema 决定（Plutus V3 中 spend datum 为 optional）
- **trace**: mint 的 ScriptContext 没有 own_ref，有 mint 字段；withdrawal 有 credential
- **redeemer-index**: tag 不同（`spend:0` vs `mint:0` vs `withdrawal:0`）
- **tx build**: script_inputs vs mint vs withdrawals 是不同的交易构造路径
- **diagnose**: 不同 purpose 有不同的失败模式和错误码

涉及命令：`schema validate`、`trace`、`redeemer-index`、`tx build`、`diagnose`

---

## 配置文件 `.utxray.toml`

```toml
[network]
default = "preview"

[backend]
primary = "ogmios"
query = "blockfrost"              # UTXO / datum 查询
evaluator = "ogmios"              # ExUnits 评估
simulator = "local-ledger"        # 完整 phase-1 校验

[ogmios]
host = "127.0.0.1"
port = 1337

[blockfrost]
project_id = "previewXXXXXXXXXXX"

[agent]
context_path = ".utxray/context.json"
auto_update_context = true        # build(ok) 后更新；test 完成并产出摘要后更新（含 mixed）

[defaults]
trace_level = "verbose"
format = "json"
include_raw = false               # 默认不内嵌大字段
```

---

## 输出协议约定

### 版本化

所有命令的 JSON 输出顶层包含 `"v"` 字段，标识输出 schema 版本：

```json
{ "v": "0.1.0", "status": "ok", "..." }
```

兼容策略：
- 新增字段：不破坏旧版消费者（agent 应忽略未知字段）
- 字段弃用：先标记 `deprecated: true` 保留至少一个 minor 版本，再移除
- 重大变更（字段改名/删除/语义变化）：递增 major 版本
- agent 判断升级：当 `v` 的 major 版本大于自身已知的最大 major 版本时，应提示用户升级

### 时间单位约定

| 字段名 | 单位 | 说明 |
|---|---|---|
| `posix_time_ms` | 毫秒 | 与 Aiken 一致（如 `1672843961000`） |
| `unix_time_s` | 秒 | 仅在同时提供 ms 时作为便利字段 |
| `slot` | slot number | 整数，网络相关。**例外：** `slot` 已是无歧义标识符，允许裸用 |

所有时间类输入输出字段名**必须包含单位后缀**（`_ms` / `_s`），避免歧义。

### 量纲约定

| 领域 | 字段示例 | 单位 | 说明 |
|---|---|---|---|
| 价值 | `fee`, `total_in`, `total_out`, `lovelace` | lovelace | 1 ADA = 1,000,000 lovelace |
| 执行预算 | `exec_units.cpu` | UPLC machine steps | 原始 CEK machine step count |
| 执行预算 | `exec_units.mem` | UPLC memory units | 原始 CEK memory unit count |
| 脚本大小 | `size_bytes` | bytes | 编译后 CBOR 字节数 |

### 状态字段约定

| 字段 | 值域 | 使用位置 | 含义 |
|---|---|---|---|
| `status` | `ok` \| `error` \| `mixed` | 所有命令的顶层 | 整体操作结果 |
| `result` | `pass` \| `fail` | 子项（单个 test、单个 script、replay step） | 具体对象的验证结果 |
| `severity` | `critical` \| `warning` \| `info` | diagnose、error 详情 | 错误严重程度 |
| `confidence` | `high` \| `medium` \| `low` | diagnose | 诊断确定性 |

规则：
- 顶层永远是 `status`，子项永远是 `result`，不混用
- `mixed` = 部分子项 pass、部分 fail
- `source_command` 使用点分枚举格式：`tx.simulate`、`tx.submit`、`replay.run`

**status 硬规则（agent 必须依据此判断是否继续）：**

| `status` | 含义 | agent 应如何反应 |
|---|---|---|
| `ok` | 命令成功执行并产出可用结果（子项可能全部 pass，也可能有 fail） | 检查子项决定是否继续 |
| `mixed` | 命令成功执行，多个子项结果有 pass/fail 混合 | 检查 `result: "fail"` 的子项，决定是否修复后重试 |
| `error` | **工具本身**无法完成操作 | 不要重试同一命令，先排查原因 |

`status: "error"` 仅用于以下情况：
- 工具参数非法、文件找不到、配置缺失
- 网络不可达、backend 连接失败
- 编译失败（语法/类型错误 — 代码有问题导致工具无法产出结果）
- mainnet 安全拦截

`status: "error"` **不用于**以下情况（这些是 `ok` 或 `mixed` + 子项 `result: "fail"`）：
- 验证器返回 False → `status: "ok"`, 对应 script 子项 `result: "fail"`
- 测试有失败项 → `status: "mixed"`
- 交易模拟中脚本未通过 → `status: "ok"` 或 `"mixed"`，对应 script 子项 `result: "fail"`
- schema validate 发现 datum 字段不匹配 → `status: "ok"`，datum 子项 `valid: false`
- diagnose 成功诊断出问题 → `status: "ok"`（工具正常工作，诊断结果是产出物）

核心原则：**`error` = 工具坏了，`ok/mixed` + `result: fail` = 合约/数据有问题。**

### 大字段输出策略

默认情况下，大字段（`tx_cbor`、`raw_cbor`、`plutus_json` 内嵌等）**不**在 JSON 中内联，
而是输出到文件并返回文件路径。传入 `--include-raw` 可在 JSON 中内嵌 hex / 完整 JSON。

这避免了 agent 上下文窗口和传输带宽的浪费。

**字段内联规则表：**

| 命令 | 大字段 | 默认输出 | `--include-raw` 时 |
|---|---|---|---|
| `tx build` | 交易 CBOR | `tx_file: "./tx.unsigned"` | 额外输出 `tx_cbor: "84a6..."` |
| `tx sign` | 签名交易 CBOR | `tx_file: "./tx.signed"` | 额外输出 `tx_cbor: "84a6..."` |
| `tx evaluate` | 交易 CBOR（输入） | 从文件读取 | — |
| `tx simulate` | 交易 CBOR（输入） | 从文件读取 | — |
| `cbor decode` | 原始 CBOR | 不输出 raw | 额外输出 `raw_cbor: "d879..."` |
| `datum resolve` | 原始 CBOR | 不输出 raw | 额外输出 `raw_cbor: "d879..."` |
| `replay bundle` | plutus.json、protocol_params | **例外：默认内嵌**（自包含性优先） | — |
| `budget` / `budget compare` | — | 无大字段 | — |

字段名在两种模式下保持一致（`tx_file` 始终存在，`tx_cbor` 仅在 `--include-raw` 时追加）。

### ExUnits 来源标注

所有输出 `exec_units` 的命令同时输出 `budget_source` 字段，避免不同精度的 ExUnits 被混用：

| `budget_source` | 来源 | 精度 |
|---|---|---|
| `trace_minimal` | `trace` 模式 A（最小化上下文） | 低，偏差可达 30-50% |
| `trace_full` | `trace` 模式 B（完整上下文） | 高 |
| `tx_evaluate` | `tx evaluate`（Ogmios / evaluator） | 高 |
| `tx_simulate` | `tx simulate`（含完整交易上下文） | 高 |
| `test` | `test`（Aiken 内置测试运行器） | 中（测试上下文可能与真实交易不同） |

---

## 命令优先级

### P0 — 必做（17 个）

| # | Command | 理由 |
|---|---|---|
| 1 | `build` | 一切的起点，没有 blueprint 后续都跑不了 |
| 2 | `typecheck` | 快速迭代，3 秒出结果 vs build 的 10+ 秒 |
| 3 | `test` | 验证器逻辑验证的主路径，对齐 aiken check |
| 4 | `trace` | AI 调试核心能力：任意输入探测验证器 |
| 5 | `schema validate` | 拦截编码错误的第一道防线 |
| 6 | `cbor decode` | 链上数据 → 可读 JSON |
| 7 | `script-data-hash` | Cardano 特有的高频 bug 源 |
| 8 | `redeemer-index` | input 排序导致的 redeemer 错位 |
| 9 | `tx build` | 构建交易是链上交互的唯一入口 |
| 10 | `tx evaluate` | 拿到 ExUnits 才能设置 redeemer budget |
| 11 | `tx simulate` | 完整交易验证，Phase-1 + Phase-2 |
| 12 | `utxo query` | 查链上状态，构建交易的前提 |
| 13 | `datum resolve` | 配合 utxo query 解读脚本锁定的资产 |
| 14 | `replay bundle` | 打包失败现场 |
| 15 | `replay run` | 重放 bundle，确认修复有效 |
| 16 | `diagnose` | 把零散工具串成闭环的粘合剂 |
| 17 | `env` | 所有命令的前提：确认工具链就绪 |

### P1 — 很重要（11 个）

| Command | 理由 |
|---|---|
| `auto` | 高层工作流入口，自动编排 P0 命令链，降低 agent 编排成本 |
| `cbor diff` | 对比两个 datum/redeemer 的结构差异 |
| `context params` | 查 cost model、max tx size 等 |
| `context tip` | 查 slot/time 映射，含 slot ↔ POSIX 双向转换 |
| `tx sign` | 完整 tx 生命周期 |
| `tx submit` | 提交测试网 |
| `utxo diff` | 对比交易前后 UTXO 变化 |
| `replay diff` | 修复前后对比 |
| `budget` | 资源消耗分析 |
| `budget compare` | 重构后量化 CPU/mem 变化 |
| `gen-context` | AI agent 的项目概览 |

### P2 — 可后放（6 + 1 deferred）

| Command | 理由 |
|---|---|
| `blueprint` | build 已生成 blueprint，单独解析是便利功能 |
| `blueprint apply` | 只有参数化验证器才需要 |
| `blueprint convert` | 将 blueprint 转换为 cardano-cli 兼容格式 |
| `uplc eval` | 直接跑 UPLC 是低频操作 |
| `test-sequence` | 多 tx 状态机测试，实现复杂度高 |
| `scaffold test` | AI 自己能写测试桩 |
| `test-watch` ⏸️ | 已 deferred，对 AI 无直接价值 |

### 统计

| Category | Active | Deferred | Total |
|---|---|---|---|
| Static analysis | 7 | 0 | 7 |
| Automated testing | 3 | 1 | 4 |
| Script probe | 2 | 0 | 2 |
| CBOR / Schema | 5 | 0 | 5 |
| Tx lifecycle | 5 | 0 | 5 |
| Chain context | 5 | 0 | 5 |
| Replay | 3 | 0 | 3 |
| Performance | 2 | 0 | 2 |
| Diagnosis | 1 | 0 | 1 |
| Workflow | 1 | 0 | 1 |
| **Total** | **34** | **1** | **35** |

---

## 零、Workflow（1 个命令）

> 高层入口，自动编排底层命令，降低 agent 选择成本。

### `utxray auto`

自动调试工作流。根据场景自动编排 P0 命令链。

```bash
utxray auto --validator <n> --purpose <purpose>
            [--scenario build|test|trace|tx|full]   # 默认 full
            [--datum <json|file>]
            [--redeemer <json|file>]
            [--tx-spec <json|file>]
```

场景说明：
- `build`: typecheck → build（不需要 datum/redeemer 输入）
- `test`: build → test → diagnose（如果有失败）
- `trace`: build → schema validate → trace → diagnose（如果失败）
- `tx`: build → tx build → tx evaluate → tx build（二遍）→ tx simulate → diagnose（如果失败）
- `full`: build → test → trace → tx build → tx evaluate → tx build → tx simulate → 如果失败则 diagnose + replay bundle

**工件传递规则：** 每个 step 的输出文件自动作为下一个 step 的输入。具体传递链：
- `build` → 产出 `plutus.json`，后续所有命令自动引用
- `tx build`（第一遍）→ 产出 `tx_file`，传递给 `tx evaluate`
- `tx evaluate` → 产出 `eval_result_file`，传递给 `tx build`（第二遍）的 `--exec-units`
- `tx build`（第二遍）→ 产出 `tx_file`，传递给 `tx simulate`
- 失败时 → 最后一个失败 step 的输出传递给 `diagnose --from`

所有中间文件写入 `.utxray/auto/` 目录，文件名包含 step 编号（如 `01_build.json`、`09_tx_build_draft.json`）。

**停止条件（保守模式，无例外）：**
- `status: "error"`（工具本身出错）→ 立即停止
- `status: "ok"` 且无任何失败信号 → 继续下一步
- `status: "ok"` 或 `"mixed"` 且出现任一失败信号 → 执行 `diagnose`，若 `scenario=full` 则追加 `replay bundle`，然后**停止**
- 失败信号包括：`result: "fail"`、`datum.valid: false`、`redeemer.valid: false`

不提供"跳过失败继续执行"例外。理由：带着已知失败继续跑后续步骤会叠加混淆，diagnose 的归因难度上升。agent 修完代码后重跑 `auto` 是更稳的循环。

**输出：**

```json
{
  "v": "0.1.0",
  "status": "mixed",
  "scenario": "test",
  "steps": [
    { "command": "build", "status": "ok", "duration_ms": 320 },
    { "command": "test", "status": "mixed", "summary": { "passed": 3, "failed": 2 } },
    { "command": "diagnose", "status": "ok", "error_code": "PHASE2_SCRIPT_FAIL", "confidence": "high" }
  ],
  "stopped_at": "diagnose",
  "reason": "Test failures detected. Fix the issues identified by diagnose before proceeding.",
  "artifacts_dir": ".utxray/auto/",
  "suggested_next": "utxray test --match 'cannot_unlock_after_deadline'"
}
```

---

## 一、Static analysis（7 个命令）

> 代码正确性，编译前检查。全部纯本地运行。

### `utxray build`

```bash
utxray build [--watch]
```

**输出（status: ok）：**

```json
{
  "v": "0.1.0",
  "status": "ok",
  "validators": [
    {
      "name": "escrow.spend", "purpose": "spend", "hash": "a1b2c3...",
      "address": "addr_test1wq...", "plutus_version": "v3", "size_bytes": 1024
    },
    {
      "name": "token.mint", "purpose": "mint", "hash": "d4e5f6...",
      "policy_id": "d4e5f6...", "plutus_version": "v3", "size_bytes": 512
    }
  ],
  "blueprint_path": "./plutus.json",
  "compile_time_ms": 320
}
```

**输出（status: error — 编译失败）：**

```json
{
  "v": "0.1.0",
  "status": "error",
  "errors": [
    {
      "severity": "critical", "code": "E0001",
      "file": "validators/escrow.ak", "line": 42, "col": 12,
      "message": "Type mismatch: expected Bool, got Int",
      "snippet": "let result = amount + 1",
      "hint": "Did you mean to use a comparison operator like '>' or '=='?"
    }
  ]
}
```

### `utxray typecheck`

```bash
utxray typecheck [--module <n>]
```

### `utxray blueprint`

```bash
utxray blueprint [--file <path>]
```

**输出：**

```json
{
  "v": "0.1.0",
  "status": "ok",
  "preamble": { "title": "my-project", "version": "0.1.0", "plutus_version": "v3", "compiler": "aiken v1.1.17" },
  "validators": [
    {
      "name": "escrow.spend", "purpose": "spend", "hash": "a1b2c3d4...", "address": "addr_test1wq...",
      "datum_schema": {
        "type": "constructor", "index": 0, "required": true,
        "fields": [
          { "name": "owner", "type": "VerificationKeyHash" },
          { "name": "deadline", "type": "POSIXTime" },
          { "name": "amount", "type": "Int" }
        ]
      },
      "redeemer_schema": {
        "type": "enum",
        "variants": [{ "name": "Unlock", "index": 0, "fields": [] }, { "name": "Cancel", "index": 1, "fields": [] }]
      },
      "parameters": []
    },
    {
      "name": "token.mint", "purpose": "mint", "hash": "d4e5f6...", "policy_id": "d4e5f6...",
      "datum_schema": null,
      "redeemer_schema": {
        "type": "enum",
        "variants": [{ "name": "Mint", "index": 0, "fields": [] }, { "name": "Burn", "index": 1, "fields": [] }]
      },
      "parameters": []
    }
  ]
}
```

### `utxray blueprint apply`

```bash
utxray blueprint apply --validator <n> --params <json>
```

### `utxray blueprint convert`

```bash
utxray blueprint convert --validator <n> [--out <file>]
```

### `utxray env`

```bash
utxray env [--check-drift <network>]
```

### `utxray gen-context`

```bash
utxray gen-context [--output <path>]
```

> **自动更新规则：** 当 `.utxray.toml` 中 `auto_update_context = true` 时：
> - `utxray build` 在 `status: "ok"` 后自动覆写 context 文件
> - `utxray test` 在命令成功执行并产出测试摘要后自动覆写（包括 `status: "mixed"`）
>
> 即只要工具本身没有 `status: "error"`，context 就会更新，确保 agent 始终拿到最新状态。

---

## 二、Automated testing（3 + 1 deferred）

### `utxray test`

```bash
utxray test [--match <pattern>]
            [--module <n>]
            [--trace-level <lvl>]    # silent | compact | verbose
            [--seed <n>]
            [--max-examples <n>]
```

**输出：**

```json
{
  "v": "0.1.0",
  "status": "mixed",
  "summary": { "total": 5, "passed": 3, "failed": 2 },
  "results": [
    {
      "name": "can_unlock_with_correct_signature", "module": "validators/escrow",
      "result": "pass",
      "exec_units": { "cpu": 1192183, "mem": 32451 }, "budget_source": "test",
      "traces": ["checking redeemer type: Unlock", "verifying signature: ok", "checking deadline: slot 100 <= 200, ok"]
    },
    {
      "name": "cannot_unlock_after_deadline", "module": "validators/escrow",
      "result": "fail",
      "exec_units": { "cpu": 892401, "mem": 28100 }, "budget_source": "test",
      "traces": ["checking redeemer type: Unlock", "verifying signature: ok", "checking deadline: FAIL — current_slot 300 > deadline 200"],
      "error_detail": "Test returned False",
      "failing_input": { "redeemer": { "constructor": 0, "fields": [] }, "context_slot": 300 }
    },
    {
      "name": "prop_value_preserved", "module": "validators/escrow",
      "result": "fail", "kind": "property", "iterations": 47, "seed": 12345,
      "counterexample": { "input_value": 0, "redeemer": "Withdrawal" },
      "traces": ["input value: 0 lovelace", "FAIL: output value mismatch"]
    }
  ]
}
```

### `utxray test-sequence`

```bash
utxray test-sequence --spec <sequence-spec.json>
```

### `utxray scaffold test`

> 仅生成 datum/redeemer 样板和正例/反例框架。**不生成业务逻辑断言。**

```bash
utxray scaffold test --validator <n>
```

### `utxray test-watch` ⏸️

> 已 deferred。

---

## 三、Script probe（2 个命令）

### `utxray trace`

```bash
utxray trace --validator <n>
             --purpose <purpose>
             --redeemer <json|file>
             [--datum <json|file>]         # spend: 由 blueprint schema 决定是否必需
             [--context <json|file>]       # 完整 ScriptContext（模式 B）
             [--tx-spec <json|file>]       # --context 的别名
             [--slot <n>]                  # 模拟当前 slot（模式 A）
             [--signatories <hex,...>]     # PubKeyHash 列表，每个 28 字节（56 hex 字符），长度不对则报错
```

#### 输入模式 A：最小化上下文（默认）

**spend 示例：**

```json
{
  "validator": "escrow.spend", "purpose": "spend",
  "datum": { "owner": "aabb...", "deadline": 1000, "amount": 5000000 },
  "redeemer": { "constructor": 0, "fields": [] },
  "context": { "slot": 500, "signatories": ["aabb..."], "own_ref": "abc123#0" }
}
```

**mint 示例：**

```json
{
  "validator": "token.mint", "purpose": "mint",
  "redeemer": { "constructor": 0, "fields": [] },
  "context": { "slot": 500, "signatories": ["aabb..."], "mint": { "d4e5f6...": { "MyToken": 1 } } }
}
```

#### 输入模式 B：完整上下文

传入完整 Transaction 结构（格式见 tx build 的 tx-spec.json）。

**输出（模式 A，验证器失败）：**

```json
{
  "v": "0.1.0",
  "status": "ok",
  "scope": "script_only",
  "validator": "escrow.spend", "purpose": "spend",
  "context_mode": "minimal",
  "auto_filled_fields": ["inputs", "outputs", "fee", "validity_range", "mint"],
  "cost_fidelity": "low",
  "result": "fail",
  "exec_units": { "cpu": 1050000, "mem": 30200 }, "budget_source": "trace_minimal",
  "traces": ["redeemer: Unlock", "checking owner signature: PASS", "checking deadline: current_slot=500, deadline=200", "FAIL: deadline exceeded"],
  "error_detail": "Validator returned False"
}
```

**输出（模式 B，验证器通过）：**

```json
{
  "v": "0.1.0",
  "status": "ok",
  "scope": "script_only",
  "validator": "escrow.spend", "purpose": "spend",
  "context_mode": "full",
  "auto_filled_fields": [],
  "cost_fidelity": "high",
  "result": "pass",
  "exec_units": { "cpu": 1120000, "mem": 31400 }, "budget_source": "trace_full",
  "traces": ["..."]
}
```

> **cost_fidelity 说明：** `"low"` = 模式 A，ExUnits 可能与真实交易偏差 30-50%。
> 不要将 `trace_minimal` 来源的 ExUnits 直接用作 redeemer budget。

### `utxray uplc eval`

```bash
utxray uplc eval <file.uplc> [--args <json>]
```

---

## 四、CBOR / Schema（5 个命令）

### `utxray schema validate`

```bash
utxray schema validate \
  --validator <n> \
  --purpose <purpose> \
  [--datum <json|file>] \        # spend: 由 blueprint schema 决定是否必需
  --redeemer <json|file>
```

**输出（spend，datum 验证失败）：**

```json
{
  "v": "0.1.0",
  "status": "ok",
  "purpose": "spend",
  "datum": {
    "valid": false, "required_by_schema": true,
    "errors": [{ "field": "owner", "expected": "ByteArray (28 bytes)", "got": "ByteArray (32 bytes)", "hint": "VerificationKeyHash is 28 bytes (blake2b-224), not 32." }]
  },
  "redeemer": { "valid": true, "matched_type": "escrow.Redeemer.Unlock", "constructor_index": 0 }
}
```

> 注意 `status: "ok"`：工具本身正常执行，`datum.valid: false` 表示数据问题，不是工具错误。

### `utxray cbor decode`

```bash
utxray cbor decode --hex <cbor_hex> [--schema <validator.TypeName>]
```

### `utxray cbor diff`

```bash
utxray cbor diff --left <hex|file> --right <hex|file>
```

### `utxray script-data-hash`

```bash
utxray script-data-hash \
  --redeemers <json|file> \
  --datums <json|file> \
  --cost-models <json|file|"from-network">
```

### `utxray redeemer-index`

```bash
utxray redeemer-index --tx <cbor|file>
```

**输出：**

```json
{
  "v": "0.1.0",
  "status": "ok",
  "sorted_inputs": [
    { "index": 0, "utxo": "abc123#0", "type": "pubkey" },
    { "index": 1, "utxo": "abc123#2", "type": "pubkey" },
    { "index": 2, "utxo": "def456#1", "type": "script", "validator": "escrow.spend" }
  ],
  "redeemers": [
    { "tag": "spend", "index": 2, "targets_utxo": "def456#1", "validator": "escrow.spend" },
    { "tag": "mint", "index": 0, "targets_policy": "d4e5f6...", "validator": "token.mint" }
  ],
  "sort_rules": {
    "inputs": "Sorted lexicographically by (tx_hash, output_index). tx_hash is lowercase hex string comparison. output_index is integer comparison.",
    "mint": "Sorted lexicographically by policy_id. policy_id is lowercase hex string comparison.",
    "input_normalization": "All tx_hash and policy_id values are auto-normalized to lowercase hex on input. output_index is always parsed as integer, never string. Agent can safely compare output values without additional normalization."
  }
}
```

---

## 五、Tx lifecycle（5 个命令）

### `utxray tx build`

```bash
utxray tx build --spec <tx-spec.json>
                [--exec-units <evaluate-result.json>]   # 第二遍构建时传入精确 ExUnits
```

**tx-spec.json：**

```json
{
  "inputs": [{ "utxo": "abc123#0", "type": "pubkey" }],
  "script_inputs": [
    {
      "utxo": "def456#1", "validator": "escrow.spend", "purpose": "spend",
      "datum": { "owner": "aabb...", "deadline": 1000, "amount": 5000000 },
      "redeemer": { "constructor": 0, "fields": [] }, "datum_source": "inline"
    }
  ],
  "reference_inputs": [{ "utxo": "ref789#0", "purpose": "reference_script" }],
  "outputs": [
    { "address": "addr_test1qz...", "value": { "lovelace": 5000000 }, "datum": null },
    {
      "address": "addr_test1wq...",
      "value": { "lovelace": 2000000, "tokens": { "d4e5f6...": { "MyToken": 1 } } },
      "datum": { "inline": { "owner": "ccdd...", "deadline": 2000, "amount": 2000000 } }
    }
  ],
  "mint": {
    "d4e5f6...": { "assets": { "MyToken": 1 }, "redeemer": { "constructor": 0, "fields": [] }, "validator": "token.mint" }
  },
  "withdrawals": [],
  "certificates": [],
  "collateral": "abc123#2",
  "change_address": "addr_test1qz...",
  "required_signers": ["aabb..."],
  "validity": { "from_slot": null, "to_slot": 2000 },
  "metadata": null
}
```

> **两遍构建（2-Pass Build）：** Cardano 交易需要平衡——ExUnits 影响 fee，fee 影响找零。
> 典型流程：第一遍 `tx build` 生成草稿 → `tx evaluate` 拿到精确 ExUnits →
> 第二遍 `tx build --exec-units eval-result.json` 带入 ExUnits 重新构建。
> Agent 不应跳过第二遍直接签名提交。
>
> **ExUnits 来源校验：** 当 `--exec-units` 文件中 `budget_source` 为 `trace_minimal` 时，
> `tx build` 输出 `severity: "warning"` 提示 ExUnits 精度不足：
> `"Low fidelity ExUnits from trace_minimal. Consider using tx evaluate for accurate budget."`
> 不会硬拦截（agent 可能在快速迭代），但 agent 应在最终提交前切换到高精度来源。

**输出：**

```json
{
  "v": "0.1.0",
  "status": "ok",
  "tx_file": "./tx.unsigned",
  "summary": {
    "inputs_count": 2, "outputs_count": 2,
    "scripts_invoked": [
      { "name": "escrow.spend", "purpose": "spend" },
      { "name": "token.mint", "purpose": "mint" }
    ],
    "total_input_lovelace": 15000000,
    "total_output_lovelace": 14800000,
    "estimated_fee": 200000
  }
}
```

### `utxray tx evaluate`

> 通过 Ogmios / local evaluator。Blockfrost 仅作 proxy adapter。

```bash
utxray tx evaluate --tx <cbor|file>
```

**输出：**

```json
{
  "v": "0.1.0",
  "status": "ok",
  "evaluation_only": true,
  "phase1_checked": false,
  "budget_source": "tx_evaluate",
  "redeemers": [
    { "tag": "spend", "index": 2, "validator": "escrow.spend", "exec_units": { "cpu": 1050000, "mem": 30200 }, "budget_pct": { "cpu": 0.01, "mem": 0.3 } },
    { "tag": "mint", "index": 0, "validator": "token.mint", "exec_units": { "cpu": 420000, "mem": 12800 }, "budget_pct": { "cpu": 0.004, "mem": 0.13 } }
  ],
  "total_fee_estimate": 280000,
  "min_fee": 220000
}
```

### `utxray tx simulate`

```bash
utxray tx simulate --tx <cbor|file>
                    [--backend <n>]                # ogmios-eval | local-ledger
                    [--additional-utxo <json>]
                    [--slot <n>]
```

**输出（脚本失败 — 注意 status 是 ok，子项 result 是 fail）：**

```json
{
  "v": "0.1.0",
  "status": "ok",
  "backend": "ogmios-eval",
  "is_balanced": true,
  "is_signed": false,
  "submit_ready": false,
  "phase1_check": "partial",
  "phase2_check": true,
  "scripts": [
    {
      "validator": "escrow.spend", "purpose": "spend", "input_utxo": "def456#1",
      "result": "fail",
      "exec_units": { "cpu": 1050000, "mem": 30200 }, "budget_source": "tx_simulate",
      "traces": ["redeemer: Unlock", "checking owner signature: PASS", "deadline check: FAIL — slot 500 > deadline 200"]
    }
  ],
  "balance_check": { "ok": true, "total_in": 15000000, "total_out": 14800000, "fee": 200000 }
}
```

### `utxray tx sign`

v1 仅支持本地 key file。后续版本计划支持 `--witness-file`。

```bash
utxray tx sign --tx <cbor|file> --signing-key <skey-file> [--out <file>]
```

### `utxray tx submit`

**Mainnet 需要显式确认。**

```bash
utxray tx submit --tx <cbor|file>
utxray tx submit --tx <cbor|file> --allow-mainnet   # mainnet 必须显式传入
```

**输出（status: ok）：**

```json
{
  "v": "0.1.0",
  "status": "ok",
  "tx_hash": "abc123...",
  "network": "preview",
  "slot_submitted": 82345700,
  "next_query_delay_s": 20,
  "hint": "Transaction submitted to mempool. Wait ~20s before querying updated UTXOs."
}
```

**输出（mainnet 未确认 — 这是工具拦截，所以 status: error）：**

```json
{
  "v": "0.1.0",
  "status": "error",
  "error_code": "MAINNET_SAFETY_BLOCK",
  "severity": "critical",
  "message": "Refusing to submit to mainnet without --allow-mainnet flag."
}
```

---

## 六、Chain context（5 个命令）

### `utxray utxo query`

```bash
utxray utxo query --address <addr>
utxray utxo query --validator <n>
utxray utxo query --tx <txhash> --index <n>
                  [--with-datum]
```

### `utxray utxo diff`

```bash
# 模式 A：基于交易（所有 backend）
utxray utxo diff --address <addr> --before-tx <txhash> --after-tx <txhash>

# 模式 B：基于 slot（需要 indexer 或 local-ledger）
utxray utxo diff --address <addr> --before-slot <slot> --after-slot <slot>
```

### `utxray datum resolve`

```bash
utxray datum resolve --hash <datum-hash>
utxray datum resolve --cbor <hex> [--schema <validator.TypeName>]
```

`source` 值域：`"inline_datum"` | `"witness"` | `"indexer"` | `"local_cbor"` | `"unresolved"`

### `utxray context params`

```bash
utxray context params
```

### `utxray context tip`

```bash
utxray context tip
utxray context tip --slot-to-posix <slot>
utxray context tip --time-to-slot <iso|unix_ms>
utxray context tip --range "+5m"
```

**输出（--slot-to-posix）：**

```json
{
  "v": "0.1.0",
  "status": "ok",
  "input_slot": 82345678,
  "posix_time_ms": 1742475600000,
  "iso_time": "2025-03-20T11:00:00Z",
  "within_stability_window": true,
  "conversion_confidence": "high",
  "era_summary": {
    "era": "Conway", "start_slot": 80000000, "start_time_ms": 1740130000000,
    "slot_length_ms": 1000, "system_start": "2022-11-01T00:00:00Z",
    "source": "ogmios queryLedgerState/eraSummaries"
  }
}
```

**输出（远未来时间 — conversion_confidence: low）：**

```json
{
  "v": "0.1.0",
  "status": "ok",
  "input_time": "2027-06-01T00:00:00Z",
  "input_posix_ms": 1811894400000,
  "slot": 151845678,
  "within_stability_window": false,
  "conversion_confidence": "low",
  "warning": "Conversion beyond stability window. A future hard fork may change slot length, invalidating this result.",
  "era_summary": { "era": "Conway", "slot_length_ms": 1000, "source": "ogmios queryLedgerState/eraSummaries" }
}
```

---

## 七、Replay（3 个命令）

### `utxray replay bundle`

> Bundle 自包含。默认内嵌 plutus.json 和 protocol params（例外——自包含性优先于大字段策略）。

```bash
utxray replay bundle --from <result.json|tx-error.json> [--tx <cbor|file>] [--output <file>]
```

**Bundle 结构：**

```json
{
  "v": "0.1.0",
  "created_at": "2025-03-20T10:30:00Z",
  "build_artifacts": {
    "aiken_version": "1.1.17",
    "plutus_json": { "preamble": {}, "validators": ["...完整 blueprint..."] },
    "aiken_toml": { "name": "my-escrow", "version": "0.1.0", "plutus_version": "v3", "dependencies": [{ "name": "aiken-lang/stdlib", "version": "v2.2.0" }] },
    "trace_level": "verbose", "build_mode": "check",
    "script_hash": "a1b2c3d4...", "source_revision": "git:abc1234"
  },
  "chain_snapshot": {
    "network": "preview", "era": "Conway", "slot": 82345678,
    "protocol_params": { "min_fee_coefficient": 44, "cost_models": { "PlutusV3": ["...完整 cost model..."] }, "max_tx_ex_units": { "cpu": 10000000000, "mem": 10000000 } },
    "utxo_set": [{ "tx_hash": "abc123", "index": 0, "value": { "lovelace": 5000000 }, "address": "addr_test1wq...", "datum": { "inline": {} } }]
  },
  "execution": {
    "command": "tx.simulate",
    "args": { "tx_file": "./tx.unsigned", "backend": "ogmios-eval" },
    "result": {
      "status": "ok",
      "scripts": [{ "validator": "escrow.spend", "result": "fail", "traces": ["..."] }]
    }
  }
}
```

### `utxray replay run`

```bash
utxray replay run --bundle <file>
```

### `utxray replay diff`

```bash
utxray replay diff --before <result1.json> --after <result2.json>
```

---

## 八、Performance（2 个命令）

### `utxray budget`

```bash
utxray budget --validator <n>
utxray budget --all
```

### `utxray budget compare`

```bash
utxray budget compare --before <blueprint.json> --after <blueprint.json> --validator <n>
```

---

## 九、Diagnosis（1 个命令）

### `utxray diagnose`

```bash
utxray diagnose --from <file>
utxray diagnose --from -                                        # stdin
utxray tx simulate --tx ./tx.cbor | utxray diagnose --from -
```

**输出：**

```json
{
  "v": "0.1.0",
  "status": "ok",
  "error_code": "REDEEMER_INDEX_MISMATCH",
  "severity": "critical",
  "category": "cbor_schema",
  "confidence": "high",
  "source_command": "tx.simulate",
  "matched_rules": [
    "redeemer tag=spend index=0 does not match sorted input position of target validator",
    "escrow.spend is at sorted index 2, not 0"
  ],
  "summary": "Redeemer at index 0 targets input abc123#2, but validator escrow.spend is attached to input def456#1 (sorted index 2).",
  "evidence": {
    "expected_index": 2, "actual_index": 0,
    "input_sort_order": ["abc123#0", "abc123#2", "def456#1"],
    "validator": "escrow.spend", "purpose": "spend", "attached_to": "def456#1"
  },
  "suggested_commands": ["utxray redeemer-index --tx ./tx.cbor", "utxray cbor decode --hex <redeemer_cbor>"],
  "related_errors": []
}
```

> **confidence:** `"high"` = 确定性推断；`"medium"` = 强关联但需验证；`"low"` = 启发式。

**错误码枚举（初始集）：**

| Error code | Category | 典型场景 |
|---|---|---|
| `TYPE_MISMATCH` | static | 编译期类型错误 |
| `SCHEMA_MISMATCH` | cbor_schema | datum/redeemer 字段不匹配 blueprint |
| `CONSTRUCTOR_INDEX_WRONG` | cbor_schema | constructor 编号错误 |
| `REDEEMER_INDEX_MISMATCH` | cbor_schema | redeemer 指向错误的 input/mint/withdrawal |
| `SCRIPT_DATA_HASH_MISMATCH` | cbor_schema | 计算的 hash 与交易中的不一致 |
| `DATUM_NOT_FOUND` | chain_context | 链上找不到对应 datum |
| `UTXO_CONSUMED` | chain_context | 目标 UTXO 已被消费 |
| `VALIDITY_INTERVAL_FAIL` | phase1 | 当前 slot 不在 validity range 内 |
| `PHASE1_BALANCE_ERROR` | phase1 | 输入输出金额不平衡 |
| `PHASE1_MIN_UTXO_FAIL` | phase1 | 输出低于最小 UTXO 值 |
| `PHASE1_COLLATERAL_MISSING` | phase1 | 缺少 collateral 输入 |
| `PHASE1_REQUIRED_SIGNER_MISSING` | phase1 | required signer 未提供 |
| `PHASE1_TX_SIZE_EXCEEDED` | phase1 | 交易超过最大允许尺寸 |
| `PHASE2_SCRIPT_FAIL` | phase2 | 验证器返回 False |
| `PHASE2_BUDGET_EXCEEDED` | phase2 | 脚本执行超出 ExUnits 限制 |
| `PHASE2_SCRIPT_ERROR` | phase2 | 脚本执行时异常（如 expect 失败） |
| `MINT_POLICY_FAIL` | phase2 | Minting policy 验证失败 |
| `WITHDRAWAL_SCRIPT_FAIL` | phase2 | Withdrawal 脚本验证失败 |
| `CERT_SCRIPT_FAIL` | phase2 | Certificate 脚本验证失败 |
| `SUBMIT_ALREADY_SPENT` | submit | 输入已被消费 |
| `SUBMIT_NETWORK_ERROR` | submit | 网络不可达 |
| `MAINNET_SAFETY_BLOCK` | submit | 未传 --allow-mainnet |
| `UNKNOWN_ERROR` | fallback | 未匹配任何已知模式。仍提供 evidence 和 suggested_commands |

---

## Backend Capability Matrix

> 标注 ✅ 的项已通过官方文档验证（Aiken CLI、Ogmios API、Plutus V3 spec）。
> 标注 `⚠️ proxy` / `⚠️ limited` 的项为 utxray adapter 设计能力，
> 不一定是 backend 原生能力，实现时需逐项校正。

| Capability | Ogmios | Local ledger / devnet | Blockfrost |
|---|---|---|---|
| Script evaluation (ExUnits) | ✅ `evaluateTransaction` | ✅ native | ⚠️ proxy to Ogmios |
| Phase-1 ledger rules | ❌ partial only | ✅ full | ❌ |
| Phase-2 script validation | ✅ | ✅ | ⚠️ proxy |
| Balance check | ❌ | ✅ | ❌ |
| Tx submission | ✅ `submitTransaction` | ✅ | ✅ |
| UTXO query (current) | ⚠️ limited (by address) | ✅ full ledger state | ✅ rich API |
| UTXO query (historical by slot) | ❌ | ✅ if snapshot available | ⚠️ limited |
| Datum resolve | ❌ | ⚠️ if in UTXO set | ✅ indexer |
| Protocol params | ✅ `queryLedgerState` | ✅ | ✅ |
| Chain tip / slot | ✅ | ✅ | ✅ |
| Era summaries (slot↔time) | ✅ `eraSummaries` | ✅ | ⚠️ limited |
| Additional UTXO set | ✅ in evaluate | ✅ | ❌ |
| Tx chaining | ✅ via additionalUtxoSet | ✅ | ❌ confirmed-only |
| Setup complexity | 中 | 高 | 低（API key） |

### 命令 × Backend 依赖

| Command | Primary backend | Fallback | Offline |
|---|---|---|---|
| `build` | — | — | ✅ |
| `typecheck` | — | — | ✅ |
| `test` | — | — | ✅ |
| `trace` | — | — | ✅ |
| `schema validate` | — | — | ✅ |
| `cbor decode` | — | — | ✅ |
| `cbor diff` | — | — | ✅ |
| `script-data-hash` | — | — | ✅ |
| `redeemer-index` | — | — | ✅ |
| `tx build` | — | — | ✅ |
| `tx evaluate` | Ogmios | Blockfrost (proxy) | ❌ |
| `tx simulate` | Local ledger | Ogmios (partial) | ❌ |
| `tx sign` | — | — | ✅ |
| `tx submit` | Ogmios | Blockfrost | ❌ |
| `utxo query` | Blockfrost | Ogmios | ❌ |
| `utxo diff` (by tx) | Blockfrost | Ogmios | ❌ |
| `utxo diff` (by slot) | Local ledger | — | ❌ |
| `datum resolve` | Blockfrost | Ogmios (limited) | ⚠️ |
| `context params` | Ogmios | Blockfrost | ❌ |
| `context tip` | Ogmios | Blockfrost (limited) | ❌ |
| `replay bundle` | — | — | ✅ |
| `replay run` | — | — | ✅ |
| `replay diff` | — | — | ✅ |
| `budget` | — | — | ✅ |
| `budget compare` | — | — | ✅ |
| `diagnose` | — | — | ✅ |
| `gen-context` | — | — | ✅ |
| `blueprint convert` | — | — | ✅ |
| `auto` | Depends on scenario | — | Partial |
| `env` | All | — | ⚠️ |

### 推荐配置

**最小可用：**

```toml
[backend]
primary = "blockfrost"
evaluator = "blockfrost-proxy"

[blockfrost]
project_id = "previewXXX"
```

> `blockfrost-proxy` 的 evaluate 不含完整 Phase-1，tx simulate `phase1_check` 为 `"partial"`。

**推荐生产配置：**

```toml
[backend]
primary = "ogmios"
query = "blockfrost"
evaluator = "ogmios"
simulator = "local-ledger"

[ogmios]
host = "127.0.0.1"
port = 1337

[blockfrost]
project_id = "previewXXX"
```

---

## Agent 典型调试循环

```
 1. utxray env                              → 工具链就绪？
 2. utxray build                            → 编译通过？
 3. utxray test                             → 哪些测试失败了？trace 说了什么？
 4. utxray diagnose --from ./test-result    → 错误归因：error_code + confidence
 5. （AI 根据 diagnose 建议修改代码）
 6. utxray test --match "failing_test"      → 修复了吗？
 7. utxray trace --validator escrow.spend \ → 用真实数据探测
         --purpose spend \
         --datum '...' --redeemer '...'
 8. utxray schema validate --validator ...  → 编码正确吗？
         --purpose spend
 9. utxray tx build --spec tx.json          → 第一遍构建（草稿）
10. utxray tx evaluate --tx tx.unsigned     → 拿到精确 ExUnits
11. utxray tx build --spec tx.json \        → 第二遍构建（带入 ExUnits，平衡 fee 和找零）
         --exec-units eval-result.json
12. utxray tx simulate --tx tx.final        → 模拟通过？
13. utxray diagnose --from ./sim-result     → 如果失败，再次归因
14. utxray replay bundle --from ...         → 打包失败现场
15. （AI 修复后）
16. utxray replay run --bundle ...          → 同场景重验
17. utxray replay diff --before ... --after → 量化改进
18. utxray budget --validator ...           → 资源消耗合理？
19. utxray tx sign + tx submit              → 提交测试网（提交后等待 ~20s 再查询 UTXO）
```

或使用高层入口一键执行：

```bash
utxray auto --validator escrow.spend --purpose spend --scenario full \
            --datum '...' --redeemer '...' --tx-spec tx.json
```