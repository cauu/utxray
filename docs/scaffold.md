# utxray — Rust 项目脚手架与技术选型

> 面向 Rust 不太熟悉的团队，强调"用成熟 crate 消灭样板，只写业务逻辑"。

---

## 一、核心依赖选型

### 必选 crate

| 领域 | Crate | 作用 | 为什么选它 |
|---|---|---|---|
| CLI 框架 | `clap` (derive) | 命令行参数解析、子命令路由 | Rust 生态事实标准。derive 宏写 struct 自动生成 CLI |
| JSON 序列化 | `serde` + `serde_json` | 所有输入输出的 JSON 处理 | Rust 序列化事实标准 |
| 异步运行时 | `tokio` | HTTP 请求、WebSocket、文件 IO | Rust 异步事实标准 |
| HTTP 客户端 | `reqwest` (rustls) | Blockfrost API 调用 | 基于 tokio，API 友好 |
| WebSocket | `tokio-tungstenite` (rustls) | Ogmios JSON-RPC 通信 | 轻量，与 tokio 集成好 |
| TLS | `rustls` | 统一 TLS 后端 | 纯 Rust，无系统 OpenSSL 依赖，cross-compile 友好 |
| 错误处理 | `anyhow` + `thiserror` | `anyhow` 用于应用层，`thiserror` 用于库层 | 大幅减少错误处理挫败感 |
| CBOR | `pallas-codec` + `pallas-primitives` | Cardano CBOR 编解码、PlutusData 操作 | Cardano Rust 生态核心库 |
| 交易解析 | `pallas-traverse` | 解析完整交易结构 | 多 era 数据遍历与解析 |
| 子进程 | `tokio::process::Command` | 异步调用 Aiken CLI | 不阻塞 tokio runtime，见下方 Aiken 集成策略 |
| 哈希 | pallas 内置 blake2b | script-data-hash、datum hash 计算 | 不需要额外依赖 |
| 时间 | `chrono` | ISO 8601 解析、slot ↔ POSIX 转换 | 标准时间库 |
| 文件监听 | `notify` | `build --watch` | 跨平台文件系统事件 |
| 测试 | `assert_cmd` + `predicates` | CLI 集成测试 | 专为 CLI 工具设计 |
| 构建分发 | `cargo-dist` | 多平台二进制 + GitHub Release | 一行配置搞定 cross-compile |

> **TLS 统一策略：** 全项目统一走 `rustls`，不用系统 OpenSSL。
> `reqwest` 用 `features = ["rustls-tls"]` 并禁用 `default-tls`；
> `tokio-tungstenite` 用 `features = ["rustls-tls-native-roots"]`。
> 这避免了跨平台证书和系统依赖问题。

### 可选 crate（按需引入）

| Crate | 作用 | 何时引入 |
|---|---|---|
| `tabled` | 终端表格渲染（`--format text`） | P1 |
| `indicatif` | 进度条 | P1 |
| `tracing` + `tracing-subscriber` | 结构化日志 | P1 |

---

## 二、Aiken 集成策略

### v1：CLI 封装（async subprocess）

**v1 不直接依赖 `aiken-project` / `aiken-lang` crate。**

理由：
- `aiken-project` 的 crate 描述明确写了"see crates/cli for usage"，它更像内部接口，不是面向外部消费者的稳定 API
- Aiken 官方面对开发者强调的公开工作流是 `aiken build`、`aiken check` 这些 CLI 命令
- 团队不熟 Rust，直接绑定内部 crate 会把升级和兼容风险提前吃掉

**JSON 输出稳定性分级：**
- `aiken check`：社区明确讨论过利用其结构化输出做外部断言，**优先解析 JSON**
- `aiken build`：官方更强调其编译产出（`plutus.json`），stdout JSON 未有同等强度的稳定性保证。按"**结构化优先，文本回退**"处理，不把 stdout JSON 当稳定契约

实现方式（使用 `tokio::process::Command`，不阻塞 async runtime）：

```rust
// crates/utxray-core/src/aiken/cli.rs

use tokio::process::Command;
use anyhow::{Context, Result};

pub struct AikenCli {
    binary: String,
    project_dir: String,
}

impl AikenCli {
    pub fn new(project_dir: &str) -> Result<Self> {
        let binary = which::which("aiken")
            .context("aiken not found in PATH. Install via: curl -sSfL https://install.aiken-lang.org | bash")?
            .to_string_lossy()
            .to_string();
        Ok(Self {
            binary,
            project_dir: project_dir.to_string(),
        })
    }

    /// aiken build — 编译产出以 plutus.json 文件为准，stdout 按"结构化优先，文本回退"处理
    pub async fn build(&self) -> Result<AikenOutput> {
        let output = Command::new(&self.binary)
            .args(["build"])
            .current_dir(&self.project_dir)
            .env("NO_COLOR", "1")
            .output()
            .await
            .context("Failed to execute aiken build")?;

        parse_aiken_output(&output)
    }

    /// aiken check — 优先解析其结构化 JSON 输出
    pub async fn check(&self, module: Option<&str>, trace_level: &str) -> Result<AikenOutput> {
        let mut args = vec!["check", "--trace-level", trace_level];
        if let Some(m) = module {
            args.extend(["--match", m]);
        }

        let output = Command::new(&self.binary)
            .args(&args)
            .current_dir(&self.project_dir)
            .output()
            .await
            .context("Failed to execute aiken check")?;

        parse_aiken_output(&output)
    }
}

pub struct AikenOutput {
    pub exit_code: i32,
    pub parsed: Option<serde_json::Value>,   // 结构化解析成功时有值
    pub raw_stdout: String,                   // 始终保留原始输出
    pub raw_stderr: String,
}

fn parse_aiken_output(output: &std::process::Output) -> Result<AikenOutput> {
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    // 尝试从 stdout 解析 JSON，失败则 parsed = None（文本回退）
    let parsed = serde_json::from_str(&stdout).ok();

    Ok(AikenOutput { exit_code, parsed, raw_stdout: stdout, raw_stderr: stderr })
}
```

### v2（未来）：评估切换到 Rust API

当以下条件**同时满足**时，评估把部分能力从 CLI 封装切到 Rust API：
- 团队 Rust 熟练度提升
- `aiken-project` crate 有明确的 API 稳定性承诺或版本化策略
- utxray 需要 CLI 封装无法提供的能力（如 AST 级别的 source mapping）

切换顺序：trace（最受益于直接 API）→ test → build

### Pallas 的职责边界

Pallas 负责：
- CBOR 编解码（`pallas-codec`）
- PlutusData 结构操作（`pallas-primitives`）
- 交易结构遍历（`pallas-traverse`）— 用于 redeemer-index、script-data-hash
- 基础哈希计算（blake2b）

Pallas **不负责**：
- 交易构建（tx build 的组装逻辑需要自己实现产品层封装）
- 交易模拟（tx simulate 依赖 Ogmios/local-ledger backend）
- 高层应用逻辑（diagnose 规则引擎、auto 工作流编排）

---

## 三、项目结构

### v1：两个 crate

```
utxray/
├── Cargo.toml                  # workspace 根配置
├── Cargo.lock
├── .utxray.toml.example        # 示例配置
├── dist-workspace.toml         # cargo-dist 分发配置
│
├── crates/
│   ├── utxray-cli/             # 二进制入口（薄壳）
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── context.rs      # AppContext（配置、格式、backend）
│   │       └── commands/
│   │           ├── mod.rs
│   │           ├── build.rs
│   │           ├── typecheck.rs
│   │           ├── test.rs
│   │           ├── trace.rs
│   │           ├── schema.rs
│   │           ├── cbor.rs
│   │           ├── tx.rs
│   │           ├── utxo.rs
│   │           ├── context_cmd.rs
│   │           ├── replay.rs
│   │           ├── budget.rs
│   │           ├── diagnose.rs
│   │           ├── auto.rs
│   │           └── env.rs
│   │
│   └── utxray-core/            # 核心业务逻辑（纯库）
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── config.rs       # .utxray.toml 解析
│           ├── output.rs       # 统一输出结构
│           ├── error.rs        # 错误码枚举
│           ├── aiken/          # Aiken CLI 封装
│           │   ├── mod.rs
│           │   └── cli.rs      # subprocess 调用 + JSON 解析
│           ├── cbor/           # CBOR/Schema 操作（pallas）
│           │   ├── mod.rs
│           │   ├── decode.rs
│           │   ├── diff.rs
│           │   ├── schema.rs
│           │   └── script_data_hash.rs
│           ├── tx/             # 交易相关
│           │   ├── mod.rs
│           │   ├── builder.rs
│           │   ├── evaluator.rs
│           │   ├── simulator.rs
│           │   ├── signer.rs
│           │   └── submitter.rs
│           ├── chain/          # 链上查询
│           │   ├── mod.rs
│           │   ├── utxo.rs
│           │   ├── datum.rs
│           │   ├── params.rs
│           │   └── tip.rs
│           ├── replay/
│           │   ├── mod.rs
│           │   ├── bundle.rs
│           │   ├── runner.rs
│           │   └── diff.rs
│           ├── diagnose/
│           │   ├── mod.rs
│           │   ├── classifier.rs
│           │   └── rules.rs
│           └── backend/        # Backend 具体实现
│               ├── mod.rs
│               ├── blockfrost.rs   # v1 primary backend
│               └── ogmios.rs      # v1 或 v2
│
├── tests/                      # 集成测试
│   ├── support/                # 测试辅助（v1 不拆独立 crate）
│   │   ├── mod.rs
│   │   ├── fixtures.rs         # fixture 加载
│   │   └── mock_backend.rs     # mock chain query
│   ├── cli_build_test.rs
│   ├── cli_cbor_test.rs
│   └── fixtures/               # 测试用 Aiken 项目、blueprint、CBOR 样本
│       ├── hello_world/
│       └── escrow/
│
└── docs/
    └── spec.md                 # 定稿的接口文档
```

### 为什么不在 v1 拆 test-utils crate？

对新团队来说，多 crate 带来的心智负担（workspace 依赖、版本同步、feature 传递）
比"测试代码多一点"更伤效率。`tests/support/` 模块足够用，等辅助逻辑真的膨胀
（超过 500 行或被多个 crate 依赖）再拆。

---

## 四、关键代码模式

### 4.1 统一输出结构

```rust
// crates/utxray-core/src/output.rs

use serde::Serialize;
use crate::error::Severity;

const UTXRAY_VERSION: &str = "0.1.0";

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Error,
    Mixed,
}

/// 验证/执行结果（不叫 Result，避免与 std::Result 撞名）
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Pass,
    Fail,
}

/// 所有命令输出都通过这个包装器，保证顶层字段一致
#[derive(Serialize)]
pub struct Output<T: Serialize> {
    pub v: String,
    pub status: Status,
    #[serde(flatten)]
    pub data: T,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Warning>,
}

#[derive(Serialize)]
pub struct Warning {
    pub severity: Severity,       // 类型化枚举，不是裸字符串
    pub message: String,
}

impl<T: Serialize> Output<T> {
    pub fn ok(data: T) -> Self {
        Self {
            v: UTXRAY_VERSION.to_string(),
            status: Status::Ok,
            data,
            warnings: vec![],
        }
    }

    pub fn mixed(data: T) -> Self {
        Self {
            v: UTXRAY_VERSION.to_string(),
            status: Status::Mixed,
            data,
            warnings: vec![],
        }
    }

    pub fn error(data: T) -> Self {
        Self {
            v: UTXRAY_VERSION.to_string(),
            status: Status::Error,
            data,
            warnings: vec![],
        }
    }

    pub fn with_warning(mut self, severity: Severity, msg: impl Into<String>) -> Self {
        self.warnings.push(Warning {
            severity,
            message: msg.into(),
        });
        self
    }
}

/// 输出到 stdout，格式由 AppContext 决定
pub fn print_output<T: Serialize>(output: &Output<T>, format: &str) -> anyhow::Result<()> {
    match format {
        "json" => {
            let json = serde_json::to_string_pretty(output)?;
            println!("{json}");
        }
        "text" => {
            // P1: 实现 text 格式
            let json = serde_json::to_string_pretty(output)?;
            println!("{json}");
        }
        _ => anyhow::bail!("Unknown format: {format}"),
    }
    Ok(())
}
```

### 4.2 main.rs — 正确的错误处理

```rust
// crates/utxray-cli/src/main.rs

use clap::{Parser, Subcommand};

mod commands;
mod context;

use context::AppContext;
use utxray_core::output::{Output, Status, print_output};

#[derive(Parser)]
#[command(name = "utxray", version, about = "UTxO X-Ray — Cardano contract debugger for AI agents")]
struct Cli {
    #[arg(long, default_value = ".")]
    project: String,

    #[arg(long, default_value = "preview")]
    network: String,

    #[arg(long, default_value = "json")]
    format: String,

    #[arg(long)]
    include_raw: bool,

    #[arg(long)]
    verbose: bool,

    #[arg(long)]
    backend: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Build {
        #[arg(long)]
        watch: bool,
    },
    Typecheck {
        #[arg(long)]
        module: Option<String>,
    },
    Test(commands::test::TestArgs),
    Trace(commands::trace::TraceArgs),
    #[command(subcommand)]
    Schema(commands::schema::SchemaCommands),
    #[command(subcommand)]
    Cbor(commands::cbor::CborCommands),
    ScriptDataHash(commands::cbor::ScriptDataHashArgs),
    RedeemerIndex(commands::cbor::RedeemerIndexArgs),
    #[command(subcommand)]
    Tx(commands::tx::TxCommands),
    #[command(subcommand)]
    Utxo(commands::utxo::UtxoCommands),
    #[command(subcommand)]
    Datum(commands::utxo::DatumCommands),
    #[command(subcommand)]
    Context(commands::context_cmd::ContextCommands),
    #[command(subcommand)]
    Replay(commands::replay::ReplayCommands),
    #[command(subcommand)]
    Budget(commands::budget::BudgetCommands),
    Diagnose(commands::diagnose::DiagnoseArgs),
    #[command(subcommand)]
    Blueprint(commands::build::BlueprintCommands),
    Auto(commands::auto::AutoArgs),
    Env(commands::env::EnvArgs),
    GenContext(commands::env::GenContextArgs),
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let format = cli.format.clone();

    // 配置加载失败 → 结构化错误输出，不 panic
    let config = match utxray_core::config::load(&cli.project) {
        Ok(c) => c,
        Err(e) => {
            let output = Output::error(serde_json::json!({
                "error_code": "CONFIG_LOAD_FAILED",
                "message": format!("Failed to load .utxray.toml: {e}")
            }));
            let _ = print_output(&output, &format);
            std::process::exit(1);
        }
    };

    let ctx = AppContext::new(&cli, &config);

    // 路由到具体命令
    let result = match cli.command {
        Commands::Build { watch } => commands::build::handle(watch, &ctx).await,
        Commands::Typecheck { module } => commands::typecheck::handle(module, &ctx).await,
        Commands::Test(args) => commands::test::handle(args, &ctx).await,
        Commands::Trace(args) => commands::trace::handle(args, &ctx).await,
        Commands::Cbor(cmd) => commands::cbor::handle(cmd, &ctx).await,
        Commands::Schema(cmd) => commands::schema::handle(cmd, &ctx).await,
        Commands::Tx(cmd) => commands::tx::handle(cmd, &ctx).await,
        Commands::Utxo(cmd) => commands::utxo::handle(cmd, &ctx).await,
        Commands::Diagnose(args) => commands::diagnose::handle(args, &ctx).await,
        Commands::Replay(cmd) => commands::replay::handle(cmd, &ctx).await,
        Commands::Budget(cmd) => commands::budget::handle(cmd, &ctx).await,
        Commands::Auto(args) => commands::auto::handle(args, &ctx).await,
        Commands::Env(args) => commands::env::handle(args, &ctx).await,
        _ => todo!(),
    };

    // 统一错误处理 → 结构化输出，不 panic
    if let Err(e) = result {
        let output = Output::error(serde_json::json!({
            "error_code": "INTERNAL_ERROR",
            "message": e.to_string()
        }));
        let _ = print_output(&output, &format);
        std::process::exit(1);
    }
}
```

### 4.3 Backend：v1 先写 concrete struct，不急着抽 trait

```rust
// crates/utxray-core/src/backend/blockfrost.rs

use reqwest::Client;
use serde::Deserialize;

/// v1: Blockfrost 是唯一的 primary backend
/// 直接写 concrete struct，不抽 trait
/// v2 加 Ogmios 时再评估是否抽 trait
///
/// ⚠️ 以下 endpoint 路径、Content-Type、返回体结构为示意性伪代码。
/// 落实现前必须逐条对照 Blockfrost 官方 OpenAPI 文档：
/// https://docs.blockfrost.io/
pub struct BlockfrostBackend {
    client: Client,
    base_url: String,
    project_id: String,
}

impl BlockfrostBackend {
    pub fn new(project_id: &str, network: &str) -> anyhow::Result<Self> {
        let base_url = match network {
            "preview" => "https://cardano-preview.blockfrost.io/api/v0",
            "preprod" => "https://cardano-preprod.blockfrost.io/api/v0",
            "mainnet" => "https://cardano-mainnet.blockfrost.io/api/v0",
            _ => anyhow::bail!("Unknown network: {network}"),
        };

        // 统一使用 rustls
        let client = Client::builder()
            .use_rustls_tls()
            .default_headers({
                let mut h = reqwest::header::HeaderMap::new();
                h.insert("project_id", project_id.parse()?);
                h
            })
            .build()?;

        Ok(Self {
            client,
            base_url: base_url.to_string(),
            project_id: project_id.to_string(),
        })
    }

    pub async fn query_utxos(&self, address: &str) -> anyhow::Result<Vec<Utxo>> {
        let url = format!("{}/addresses/{}/utxos", self.base_url, address);
        let resp: Vec<BlockfrostUtxo> = self.client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        // 转换为 utxray 内部类型
        Ok(resp.into_iter().map(Into::into).collect())
    }

    pub async fn resolve_datum(&self, hash: &str) -> anyhow::Result<Option<DatumInfo>> {
        let url = format!("{}/scripts/datum/{}", self.base_url, hash);
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let data: BlockfrostDatum = resp.json().await?;
                Ok(Some(data.into()))
            }
            Ok(resp) if resp.status().as_u16() == 404 => Ok(None),
            Ok(resp) => Err(anyhow::anyhow!("Blockfrost error: {}", resp.status())),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn evaluate_tx(&self, tx_cbor: &[u8]) -> anyhow::Result<EvaluationResult> {
        let url = format!("{}/utils/txs/evaluate", self.base_url);
        let resp = self.client
            .post(&url)
            .header("Content-Type", "application/cbor")
            .body(tx_cbor.to_vec())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp)
    }

    pub async fn submit_tx(&self, tx_cbor: &[u8]) -> anyhow::Result<String> {
        let url = format!("{}/tx/submit", self.base_url);
        let resp = self.client
            .post(&url)
            .header("Content-Type", "application/cbor")
            .body(tx_cbor.to_vec())
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(resp.text().await?.trim_matches('"').to_string())
        } else {
            let err_body = resp.text().await?;
            Err(anyhow::anyhow!("Submit failed: {err_body}"))
        }
    }

    pub async fn query_params(&self) -> anyhow::Result<serde_json::Value> {
        let url = format!("{}/epochs/latest/parameters", self.base_url);
        let resp = self.client.get(&url).send().await?.error_for_status()?.json().await?;
        Ok(resp)
    }

    pub async fn query_tip(&self) -> anyhow::Result<serde_json::Value> {
        let url = format!("{}/blocks/latest", self.base_url);
        let resp = self.client.get(&url).send().await?.error_for_status()?.json().await?;
        Ok(resp)
    }
}
```

### 4.4 错误码枚举

```rust
// crates/utxray-core/src/error.rs

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    TypeMismatch,
    SchemaMismatch,
    ConstructorIndexWrong,
    RedeemerIndexMismatch,
    ScriptDataHashMismatch,
    DatumNotFound,
    UtxoConsumed,
    ValidityIntervalFail,
    Phase1BalanceError,
    Phase1MinUtxoFail,
    Phase1CollateralMissing,
    Phase1RequiredSignerMissing,
    Phase1TxSizeExceeded,
    Phase2ScriptFail,
    Phase2BudgetExceeded,
    Phase2ScriptError,
    MintPolicyFail,
    WithdrawalScriptFail,
    CertScriptFail,
    SubmitAlreadySpent,
    SubmitNetworkError,
    MainnetSafetyBlock,
    ConfigLoadFailed,
    InternalError,
    UnknownError,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetSource {
    TraceMinimal,
    TraceFull,
    TxEvaluate,
    TxSimulate,
    Test,
}
```

---

## 五、实现顺序

### Phase 0：脚手架（1-2 天）

- 初始化 workspace（2 个 crate）
- `clap` 命令路由骨架（所有命令 stub，内部 `todo!()`）
- `Output<T>` 统一输出
- `.utxray.toml` 配置加载
- CI（GitHub Actions：check + test + clippy + fmt）

跑通标志：`utxray env` 输出正确 JSON

### Phase 1：纯本地命令（1-2 周）

**里程碑：跑通 Aiken CLI 封装**（不是 crate 集成）。

12 个纯本地命令，不需要任何 backend 连接：

```
build → typecheck → test → trace → schema validate →
cbor decode → script-data-hash → redeemer-index → tx build →
diagnose → replay bundle → replay run
```

建议顺序：

1. `build` / `typecheck` — 跑通 Aiken CLI async subprocess 调用 + JSON 解析（`aiken check` 优先解析 JSON，`aiken build` 以 `plutus.json` 产出为准）
2. `test` — 解析 `aiken check` 的 trace 输出，结构化为 utxray 格式
3. `cbor decode` — 跑通 pallas CBOR 解码
4. `schema validate` — 依赖 blueprint 解析 + CBOR 解码
5. `script-data-hash` / `redeemer-index` — 依赖 pallas 交易解析
6. `trace` — 最复杂，依赖 ScriptContext 构造
7. `tx build` — 交易构建
8. `diagnose` — 错误分类规则引擎
9. `replay bundle` / `replay run` — 序列化/反序列化 + 重执行

### Phase 2：链上连接命令（1 周）

5 个需要 backend 的 P0 命令：

```
tx evaluate → tx simulate → utxo query → datum resolve → env（完整版）
```

先实现 Blockfrost backend（HTTP API，最简单）。

### Phase 3：P1 命令 + 分发（1-2 周）

- `auto` 工作流编排
- `tx sign` / `tx submit`
- `context params` / `context tip`
- `cbor diff` / `utxo diff` / `replay diff`
- `budget` / `budget compare`
- `gen-context`
- `cargo-dist` → GitHub Release 自动发版

### Phase 4（未来）：评估 backend 抽象

当 Ogmios backend 实现明确需要与 Blockfrost 共享接口时：
- 提取 trait（`ChainQuery`、`TxEvaluator`、`TxSubmitter`）
- 已有的 `BlockfrostBackend` 方法签名基本不变，只是加上 `impl Trait`

---

## 六、Cargo.toml 参考

### workspace 根

```toml
[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "Apache-2.0"
repository = "https://github.com/your-org/utxray"

[workspace.dependencies]
# Cardano
pallas-codec = "0.30"
pallas-primitives = "0.30"
pallas-traverse = "0.30"

# CLI
clap = { version = "4", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }

# HTTP / WebSocket — 统一 rustls
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
tokio-tungstenite = { version = "0.24", features = ["rustls-tls-native-roots"] }

# Utils
anyhow = "1"
thiserror = "2"
hex = "0.4"
chrono = { version = "0.4", features = ["serde"] }
which = "7"

# Testing
assert_cmd = "2"
predicates = "3"
```

### utxray-cli

```toml
[package]
name = "utxray"
version.workspace = true
edition.workspace = true

[[bin]]
name = "utxray"
path = "src/main.rs"

[dependencies]
utxray-core = { path = "../utxray-core" }
clap.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
anyhow.workspace = true

[dev-dependencies]
assert_cmd.workspace = true
predicates.workspace = true
```

### utxray-core

```toml
[package]
name = "utxray-core"
version.workspace = true
edition.workspace = true

[dependencies]
pallas-codec.workspace = true
pallas-primitives.workspace = true
pallas-traverse.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
reqwest.workspace = true
tokio-tungstenite.workspace = true
anyhow.workspace = true
thiserror.workspace = true
hex.workspace = true
chrono.workspace = true
which.workspace = true
```

---

## 七、团队 Rust 上手建议

### 最容易踩的 3 个坑和解法

**坑 1：所有权和借用**

Rust 新手 80% 的编译错误来自这里。解法：前期大量使用 `.clone()`，先让代码跑起来。
CLI 工具的性能瓶颈在 IO（网络、文件、子进程），不在内存拷贝。等团队熟悉后再优化。

**坑 2：异步函数的生命周期**

`async fn` 里引用外部变量容易报生命周期错误。解法：函数参数尽量传 owned 类型
（`String` 而不是 `&str`），或者用 `Arc<T>` 共享。

**坑 3：错误处理泛滥**

Rust 的 `Result<T, E>` 到处都是。解法：在 `utxray-cli` 层全部用 `anyhow::Result`，
一个 `?` 就能传播任何错误。只在 `utxray-core` 的公开 API 用 `thiserror` 定义精确错误类型。

### 推荐学习路径（按需即学）

1. **The Rust Book 第 1-10 章**：基础语法、所有权、struct/enum、错误处理
2. **clap derive 文档**：看懂了就能写任何 CLI 命令
3. **serde 文档**：看懂了就能做任何 JSON 序列化
4. **pallas 源码里的 examples**：看懂了就能操作 Cardano 数据结构
5. **reqwest examples**：看懂了就能调任何 HTTP API

不需要一开始就学 trait object、lifetime annotation、macro 等高级特性。遇到再学。

---

## 八、CI / CD 配置

### GitHub Actions

```yaml
name: CI
on: [push, pull_request]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo check --workspace
      - run: cargo clippy --workspace -- -D warnings
      - run: cargo test --workspace

  fmt:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --all -- --check
```

### 分发

```bash
cargo install cargo-dist
cargo dist init
# 之后每次打 tag → GitHub Actions 自动编译多平台二进制 + 生成安装脚本
```

用户安装：

```bash
curl -sSfL https://github.com/your-org/utxray/releases/latest/download/utxray-installer.sh | sh
```