# utxray — 权威参考源映射

> 本文件补充 spec.md 和 scaffold.md 的实现缺口。
> 对每个缺口，指定权威规范 + 可参考的成熟实现。
> Claude Code 在实现时应按优先级查阅：规范 > 参考实现 > 推断。

---

## 1. 交易 CBOR 构建（tx build 核心）

### 权威规范

| 内容 | 来源 | URL |
|---|---|---|
| Conway era 交易体 CDDL | cardano-ledger 官方仓库 | https://github.com/IntersectMBO/cardano-ledger/blob/master/eras/conway/impl/cddl-files/conway.cddl |
| Babbage era 交易体 CDDL（向后兼容参考） | cardano-ledger 官方仓库 | https://github.com/IntersectMBO/cardano-ledger/blob/master/eras/babbage/impl/cddl-files/babbage.cddl |
| CBOR 序列化规则（canonical encoding） | CIP-0021（HW 钱包互操作性） | https://cips.cardano.org/cip/CIP-0021 |
| Fee 计算公式 | Cardano Ledger Shelley formal spec §6 | https://github.com/IntersectMBO/cardano-ledger/tree/master/eras/shelley/formal-spec |
| 账本演进总览 | CIP-84 | https://cips.cardano.org/cip/CIP-84 |

### 参考实现（按优先级）

| 实现 | 语言 | 为什么参考 | URL |
|---|---|---|---|
| `pallas-txbuilder` | Rust | 与 utxray 同语言，直接可用作依赖或参考 | https://github.com/txpipe/pallas (crate: pallas-txbuilder，unstable feature gate) |
| `lucid-evolution` | TypeScript | Cardano 最活跃的 tx 构建库，逻辑清晰 | https://github.com/Anastasia-Labs/lucid-evolution |
| `cardano-cli` transaction build | Haskell | 官方参考实现，行为定义权威 | https://github.com/IntersectMBO/cardano-cli |
| `MeshJS` TxBuilder | TypeScript | 对 blueprint 集成好，Aiken 生态常配合使用 | https://github.com/MeshJS/mesh |

### 实现策略建议

v1 优先考虑直接使用 `pallas-txbuilder`（虽然在 unstable feature gate 后面，但功能可用）。
如果 pallas-txbuilder API 不够稳定，退化为：手动用 `pallas-primitives` 的 Conway era 类型构造交易体，CBOR 编码由 `pallas-codec` 处理。

Fee 计算公式：
```
min_fee = min_fee_coefficient × tx_size_bytes + min_fee_constant + script_exec_fee
script_exec_fee = Σ(price_memory × mem_units + price_steps × cpu_units) for each redeemer
```
参数从 protocol params 获取（`utxray context params` 的输出）。

---

## 2. script-data-hash 计算

### 权威规范

| 内容 | 来源 | URL |
|---|---|---|
| script_integrity_data 定义 | conway.cddl（搜索 `script_data_hash`） | https://github.com/IntersectMBO/cardano-ledger/blob/master/eras/conway/impl/cddl-files/conway.cddl |
| 编码规则详解（canonical CBOR、language views） | conway.cddl 注释段（搜索 `language views CDDL`） | 同上，注释中有完整算法 |

### 算法（从 CDDL 注释提取）

```
script_data_hash = blake2b_256(
    redeemers_bytes ||
    datums_bytes ||
    language_views_encoding
)

其中：
- redeemers_bytes: redeemer list 的 canonical CBOR 编码
- datums_bytes: 如果有 datums 则为其 canonical CBOR 编码，否则为空字节串
- language_views_encoding: cost models 按 language key 排序的 canonical CBOR map
  - key 排序规则：shorter key sorts first；same length → lexicographic
  - PlutusV1 cost model 编码为 integer list
  - PlutusV2/V3 cost model 编码为 integer list
```

### 参考实现

| 实现 | 文件 | URL |
|---|---|---|
| cardano-ledger (Haskell) | `Alonzo.TxBody.hs` 中的 `hashScriptIntegrity` | https://github.com/IntersectMBO/cardano-ledger （搜索 `hashScriptIntegrity`） |
| pallas | `pallas-primitives` 的 hash utilities | https://github.com/txpipe/pallas |
| lucid-evolution | `src/tx-builder/` | https://github.com/Anastasia-Labs/lucid-evolution |

---

## 3. Blockfrost API

### 权威规范

| 内容 | 来源 | URL |
|---|---|---|
| 完整 OpenAPI spec（YAML） | blockfrost/openapi 官方仓库 | https://github.com/blockfrost/openapi/blob/master/openapi.yaml |
| 可浏览的 API 文档 | Blockfrost 官方文档站 | https://docs.blockfrost.io/ |
| Rust crate（OpenAPI 生成） | crates.io | https://crates.io/crates/blockfrost-openapi |

### utxray 需要的端点映射

| utxray 命令 | Blockfrost 端点 | 方法 | 备注 |
|---|---|---|---|
| `utxo query --address` | `/addresses/{address}/utxos` | GET | 分页，默认 100 条 |
| `utxo query --validator` | 先从 blueprint 解析 address，再调上面端点 | GET | |
| `datum resolve --hash` | `/scripts/datum/{datum_hash}` | GET | 404 = unresolved |
| `tx evaluate` | `/utils/txs/evaluate` | POST | Content-Type: application/cbor |
| `tx submit` | `/tx/submit` | POST | Content-Type: application/cbor |
| `context params` | `/epochs/latest/parameters` | GET | |
| `context tip` | `/blocks/latest` | GET | |
| `env` (connectivity check) | `/` 或 `/health` | GET | |

### 注意事项

- 速率限制：10 req/s 持续，500 burst
- 分页：`?page=N&count=100&order=asc`
- 所有时间戳字段：**秒级 UNIX time**（注意：utxray 内部统一用毫秒）
- `/utils/txs/evaluate` 内部 proxy 到 Ogmios，返回格式是 Ogmios 的 EvaluateTxResponse
- 错误码：400/402/403/404/418/425/429/500

---

## 4. ScriptContext 构建（trace 核心）

### 权威规范

| 内容 | 来源 | URL |
|---|---|---|
| Plutus V3 ScriptContext 定义 | plutus-ledger-api | https://github.com/IntersectMBO/plutus/tree/master/plutus-ledger-api |
| Aiken 的 cardano/transaction 类型 | aiken-lang/stdlib | https://github.com/aiken-lang/stdlib/blob/main/lib/cardano/transaction.ak |
| 各 purpose 的上下文差异 | CIP-0069 (Plutus V3 improvements) | https://cips.cardano.org/cip/CIP-0069 |

### 参考实现

| 实现 | 为什么参考 | URL |
|---|---|---|
| Aiken 自身的测试框架 | 最权威——utxray trace 就是模拟它的行为 | https://github.com/aiken-lang/aiken/tree/main/crates/aiken-project/src |
| `pallas-applying` | Rust 实现的 phase-1/2 验证，含 ScriptContext 构建逻辑 | https://github.com/txpipe/pallas/tree/main/pallas-applying |

### 各 purpose 的 ScriptContext 差异

| Purpose | datum 参数 | 专有字段 | 来源 |
|---|---|---|---|
| `spend` | V3 中 optional（CIP-0069） | `own_ref: OutputReference` | plutus-ledger-api |
| `mint` | 无 | `own_policy: PolicyId` | plutus-ledger-api |
| `withdrawal` | 无 | `own_credential: Credential` | plutus-ledger-api |
| `certificate` | 无 | `own_credential: Credential`, `certificate: Certificate` | plutus-ledger-api |
| `propose` | 无 | `proposal_procedure: ProposalProcedure` | Conway 新增 |
| `vote` | 无 | `voter: Voter` | Conway 新增 |

### Mode A auto-fill 算法

当用户只提供 datum + redeemer + 最小 context 时，utxray 需要构造一个最小可用的 Transaction：

```
1. 创建一个 fake 脚本输入（spend）或 fake policy（mint），UTXO ref 用用户提供的 own_ref 或自动生成
2. 创建一个 pubkey 输入提供手续费（value = 足够大的 lovelace）
3. outputs = []（或一个 change output 如果需要平衡）
4. fee = 200000（硬编码默认，模式 A 不需要精确）
5. validity_range = 用户提供的 slot 转换，或 (0, +inf)
6. signatories = 用户提供的列表
7. mint = 用户提供（仅 purpose=mint 时）
8. redeemers = 构造对应 purpose 的 redeemer entry
9. 用 pallas-primitives 的 Conway era 类型组装 Transaction
10. 传入 Aiken 的 UPLC 评估器执行
```

关键：模式 A 的 auto-fill 不追求链上有效性，只追求验证器能跑起来并产出 trace。

---

## 5. Aiken CLI 输出格式

### 权威来源

| 内容 | 来源 | URL |
|---|---|---|
| aiken check 的 JSON 输出（非 TTY） | Aiken changelog（明确提及 structured JSON output） | https://github.com/aiken-lang/aiken/blob/main/CHANGELOG.md |
| aiken check --show-json-schema | Aiken CLI help | `aiken check --help` |
| aiken build 的 blueprint 格式 | CIP-0057 (Plutus Blueprint) | https://cips.cardano.org/cip/CIP-0057 |

### 解析策略

```
aiken check（测试 + trace）：
  - 非 TTY 时输出 structured JSON → 优先解析
  - 字段：test name, status (pass/fail), mem, cpu, traces
  - 不同 aiken 版本可能有字段变化 → 降级到 raw text + regex

aiken build（编译）：
  - 主要产出是 plutus.json 文件（CIP-0057 格式）
  - stdout 不保证 structured JSON → 以 plutus.json 文件为准
  - 编译错误从 stderr 解析（通常是人类可读文本）

降级策略：
  - JSON 解析失败 → 保留 raw_stdout + raw_stderr
  - raw 输出可被 diagnose 消费做文本匹配
```

### 参考

| 实现 | 为什么参考 | URL |
|---|---|---|
| Aiken VS Code 扩展 | 它也在 parse aiken CLI 输出，可以看它的解析逻辑 | https://github.com/aiken-lang/vscode-aiken |

---

## 6. Slot / POSIX 时间转换

### 权威规范

| 内容 | 来源 | URL |
|---|---|---|
| Era summaries / system start | Ogmios queryLedgerState/eraSummaries | https://ogmios.dev/api/ |
| Slot/time 关系不是简单线性 | Cardano 文档（hard fork 可能改变 slot length） | https://docs.cardano.org/ |

### 转换算法

```
给定：
  - system_start: 网络创世时间（POSIX ms）
  - era_summaries: [{start_slot, start_time_ms, slot_length_ms}, ...]

slot_to_posix(slot):
  找到 slot 所在的 era（start_slot <= slot 的最后一个 era）
  posix_ms = era.start_time_ms + (slot - era.start_slot) × era.slot_length_ms
  return posix_ms

posix_to_slot(posix_ms):
  找到 posix_ms 所在的 era（start_time_ms <= posix_ms 的最后一个 era）
  slot = era.start_slot + (posix_ms - era.start_time_ms) / era.slot_length_ms
  return floor(slot)

stability_window 判断：
  当前 era 的 stability window = 一般取 3k/f slots（k=安全参数，f=active slot coefficient）
  如果 target_slot > tip_slot + stability_window → within_stability_window = false
```

### 参考实现

| 实现 | URL |
|---|---|
| Ogmios 源码（eraSummaries 处理） | https://github.com/CardanoSolutions/ogmios |
| lucid-evolution 的 slotToUnixTime / unixTimeToSlot | https://github.com/Anastasia-Labs/lucid-evolution |

---

## 7. Diagnose 规则引擎

### 匹配规则设计

diagnose 不需要权威规范——它是 utxray 自己的产品逻辑。但匹配规则应基于已知的 Cardano 错误模式：

| Error code | 匹配信号 | 来源 |
|---|---|---|
| `SCHEMA_MISMATCH` | `datum.valid: false` 或 `redeemer.valid: false` 在 schema validate 输出中 | utxray 自身输出 |
| `REDEEMER_INDEX_MISMATCH` | redeemer-index 的 expected vs actual 不一致 | utxray 自身输出 |
| `SCRIPT_DATA_HASH_MISMATCH` | script-data-hash 计算值 ≠ 交易中的值 | utxray 自身输出 |
| `PHASE1_BALANCE_ERROR` | 交易 input 总值 ≠ output 总值 + fee | 交易解析 |
| `PHASE2_SCRIPT_FAIL` | trace 以 "Validator returned False" 结尾 | aiken check / trace 输出 |
| `PHASE2_BUDGET_EXCEEDED` | exec_units 超出 protocol params 限制 | 对比 context params |
| `VALIDITY_INTERVAL_FAIL` | current_slot 不在 validity range 内 | 对比 context tip |
| `SUBMIT_ALREADY_SPENT` | Blockfrost 返回 400 + 包含 "ValueNotConservedUTxO" 或 "BadInputsUTxO" | Blockfrost error body |

### 参考

| 来源 | URL |
|---|---|
| Cardano ledger 错误类型定义 | https://github.com/IntersectMBO/cardano-ledger（搜索 `PredFailure`） |
| Ogmios submission errors | https://ogmios.dev/api/（搜索 `SubmitTxError`） |

---

## 使用指南

Claude Code 在实现具体命令时：

1. 先查本文件对应的「权威规范」URL，读规范原文
2. 如果规范太底层难以直接实现，查「参考实现」中同语言（Rust）的实现
3. 如果 Rust 没有，查 TypeScript 参考实现（lucid-evolution / MeshJS），逻辑更可读
4. 如果发现规范与参考实现有冲突，以规范为准，记录到 docs/spec-gaps.md
5. 如果规范本身有歧义，以 cardano-ledger 仓库的 Haskell 实现为最终裁判