use clap::{Parser, Subcommand};

mod commands;
mod context;

use context::AppContext;
use utxray_core::output::{print_output, Output};

#[derive(Parser)]
#[command(
    name = "utxray",
    version,
    about = "UTxO X-Ray — Cardano contract debugger for AI agents"
)]
struct Cli {
    /// Project root directory
    #[arg(long, default_value = ".")]
    project: String,

    /// Network: preview | preprod | mainnet | local
    #[arg(long, default_value = "preview")]
    network: String,

    /// Output format: json | text
    #[arg(long, default_value = "json")]
    format: String,

    /// Include raw/large fields inline in JSON output
    #[arg(long)]
    include_raw: bool,

    /// Attach raw tool output
    #[arg(long)]
    verbose: bool,

    /// Override default backend from .utxray.toml
    #[arg(long)]
    backend: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile Aiken project
    Build {
        #[arg(long)]
        watch: bool,
    },
    /// Type-check without full build
    Typecheck {
        #[arg(long)]
        module: Option<String>,
    },
    /// Run Aiken tests
    Test(commands::test::TestArgs),
    /// Trace validator execution with custom inputs
    Trace(commands::trace::TraceArgs),
    /// Schema operations
    #[command(subcommand)]
    Schema(commands::schema::SchemaCommands),
    /// CBOR operations
    #[command(subcommand)]
    Cbor(commands::cbor::CborCommands),
    /// Compute script data hash
    ScriptDataHash(commands::cbor::ScriptDataHashArgs),
    /// Check redeemer index alignment
    RedeemerIndex(commands::cbor::RedeemerIndexArgs),
    /// Transaction operations
    #[command(subcommand)]
    Tx(commands::tx::TxCommands),
    /// UTxO operations
    #[command(subcommand)]
    Utxo(commands::utxo::UtxoCommands),
    /// Datum operations
    #[command(subcommand)]
    Datum(commands::utxo::DatumCommands),
    /// Chain context operations
    #[command(subcommand)]
    Context(commands::context_cmd::ContextCommands),
    /// Replay operations
    #[command(subcommand)]
    Replay(commands::replay::ReplayCommands),
    /// Budget analysis
    #[command(subcommand)]
    Budget(commands::budget::BudgetCommands),
    /// Diagnose transaction/test failures
    Diagnose(commands::diagnose::DiagnoseArgs),
    /// Blueprint operations
    #[command(subcommand)]
    Blueprint(commands::build::BlueprintCommands),
    /// Automated debug workflow
    Auto(commands::auto::AutoArgs),
    /// Check environment and tool versions
    Env,
    /// Generate AI agent context file
    GenContext,
}

fn print_json_error(error_code: &str, message: &str) {
    let output = Output::error(serde_json::json!({
        "error_code": error_code,
        "message": message,
    }));
    let _ = print_output(&output);
}

#[tokio::main]
async fn main() {
    // Use try_parse to handle clap errors ourselves with structured JSON
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // For help/version requests, print normally
            if e.kind() == clap::error::ErrorKind::DisplayHelp
                || e.kind() == clap::error::ErrorKind::DisplayVersion
            {
                let _ = e.print();
                std::process::exit(0);
            }
            // For all other errors (unknown command, missing args, etc.), output structured JSON
            print_json_error("INVALID_COMMAND", &e.to_string());
            std::process::exit(1);
        }
    };

    let format = cli.format.clone();

    // Config load failure -> structured JSON error + exit 1
    let config = match utxray_core::config::load(&cli.project) {
        Ok(c) => c,
        Err(e) => {
            print_json_error("CONFIG_LOAD_FAILED", &format!("{e}"));
            std::process::exit(1);
        }
    };

    let ctx = AppContext::new(
        &cli.project,
        &cli.network,
        &format,
        cli.include_raw,
        cli.verbose,
        cli.backend,
        config,
    );

    // Route to command handler
    let result = match cli.command {
        Commands::Build { watch } => commands::build::handle_build(watch, &ctx).await,
        Commands::Typecheck { module } => commands::typecheck::handle(module, &ctx).await,
        Commands::Test(args) => commands::test::handle(args, &ctx).await,
        Commands::Trace(args) => commands::trace::handle(args, &ctx).await,
        Commands::Schema(cmd) => commands::schema::handle(cmd, &ctx).await,
        Commands::Cbor(cmd) => commands::cbor::handle_cbor(cmd, &ctx).await,
        Commands::ScriptDataHash(args) => commands::cbor::handle_script_data_hash(args, &ctx).await,
        Commands::RedeemerIndex(args) => commands::cbor::handle_redeemer_index(args, &ctx).await,
        Commands::Tx(cmd) => commands::tx::handle(cmd, &ctx).await,
        Commands::Utxo(cmd) => commands::utxo::handle_utxo(cmd, &ctx).await,
        Commands::Datum(cmd) => commands::utxo::handle_datum(cmd, &ctx).await,
        Commands::Context(cmd) => commands::context_cmd::handle(cmd, &ctx).await,
        Commands::Replay(cmd) => commands::replay::handle(cmd, &ctx).await,
        Commands::Budget(cmd) => commands::budget::handle(cmd, &ctx).await,
        Commands::Diagnose(args) => commands::diagnose::handle(args, &ctx).await,
        Commands::Blueprint(cmd) => commands::build::handle_blueprint(cmd, &ctx).await,
        Commands::Auto(args) => commands::auto::handle(args, &ctx).await,
        Commands::Env => commands::env::handle(&ctx).await,
        Commands::GenContext => commands::env::handle_gen_context(&ctx).await,
    };

    // Unified error handling -> structured JSON output, never panic
    if let Err(e) = result {
        let output = Output::error(serde_json::json!({
            "error_code": "INTERNAL_ERROR",
            "message": e.to_string()
        }));
        let _ = print_output(&output);
        std::process::exit(1);
    }
}
