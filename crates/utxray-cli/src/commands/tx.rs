use clap::Subcommand;
use serde::Serialize;

use crate::context::AppContext;
use utxray_core::backend::blockfrost::BlockfrostBackend;
use utxray_core::backend::EvaluatedRedeemer;
use utxray_core::output::{print_output, Output};
use utxray_core::tx::builder;

#[derive(Subcommand, Debug)]
pub enum TxCommands {
    /// Build a transaction
    Build {
        #[arg(long)]
        spec: Option<String>,
        #[arg(long)]
        exec_units: Option<String>,
    },
    /// Evaluate transaction ExUnits
    Evaluate {
        #[arg(long)]
        tx: Option<String>,
    },
    /// Simulate a transaction (full phase-1 + phase-2 validation)
    Simulate {
        #[arg(long)]
        tx: Option<String>,
    },
    /// Sign a transaction
    Sign {
        #[arg(long)]
        tx: Option<String>,
        #[arg(long)]
        signing_key: Option<String>,
    },
    /// Submit a transaction
    Submit {
        #[arg(long)]
        tx: Option<String>,
        #[arg(long)]
        allow_mainnet: bool,
    },
}

#[derive(Debug, Serialize)]
struct TxEvaluateOutput {
    evaluation_only: bool,
    phase1_checked: bool,
    budget_source: String,
    redeemers: Vec<EvaluatedRedeemer>,
}

fn get_blockfrost(ctx: &AppContext) -> anyhow::Result<BlockfrostBackend> {
    let project_id =
        ctx.config.blockfrost.project_id.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Blockfrost project_id not configured in .utxray.toml")
        })?;
    BlockfrostBackend::new(project_id, &ctx.network)
}

/// Read CBOR hex from a --tx argument: either inline hex or a file path.
async fn read_tx_cbor(tx_arg: &str) -> anyhow::Result<String> {
    // If the argument looks like a file path (contains / or . and isn't pure hex), read the file
    let is_file = tx_arg.contains('/')
        || tx_arg.contains('\\')
        || tx_arg.ends_with(".cbor")
        || tx_arg.ends_with(".tx");
    if is_file {
        let content = tokio::fs::read_to_string(tx_arg)
            .await
            .map_err(|e| anyhow::anyhow!("failed to read tx file '{}': {}", tx_arg, e))?;
        Ok(content.trim().to_string())
    } else {
        Ok(tx_arg.to_string())
    }
}

pub async fn handle(cmd: TxCommands, ctx: &AppContext) -> anyhow::Result<()> {
    match cmd {
        TxCommands::Build { spec, exec_units } => {
            let spec_path = match spec {
                Some(p) => p,
                None => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "MISSING_ARGUMENT",
                        "message": "--spec <tx-spec.json> is required"
                    }));
                    print_output(&output)?;
                    return Ok(());
                }
            };

            let tx_output_path = builder::resolve_tx_output_path(&spec_path);
            let exec_units_ref = exec_units.as_deref();

            let output = builder::run_tx_build_safe(
                &spec_path,
                exec_units_ref,
                &tx_output_path,
                ctx.include_raw,
            );
            print_output(&output)?;
            Ok(())
        }
        TxCommands::Evaluate { tx } => {
            let tx_arg = match tx {
                Some(t) => t,
                None => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "MISSING_ARGUMENT",
                        "message": "--tx <cbor_hex_or_file> is required"
                    }));
                    print_output(&output)?;
                    return Ok(());
                }
            };

            let backend = match get_blockfrost(ctx) {
                Ok(b) => b,
                Err(e) => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "BACKEND_NOT_CONFIGURED",
                        "message": e.to_string()
                    }));
                    print_output(&output)?;
                    return Ok(());
                }
            };

            let cbor_hex = match read_tx_cbor(&tx_arg).await {
                Ok(h) => h,
                Err(e) => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "TX_READ_FAILED",
                        "message": e.to_string()
                    }));
                    print_output(&output)?;
                    return Ok(());
                }
            };

            match backend.evaluate_tx(&cbor_hex).await {
                Ok(eval_result) => {
                    let output = Output::ok(TxEvaluateOutput {
                        evaluation_only: true,
                        phase1_checked: false,
                        budget_source: "tx_evaluate".to_string(),
                        redeemers: eval_result.redeemers,
                    });
                    print_output(&output)?;
                }
                Err(e) => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "EVALUATE_FAILED",
                        "message": e.to_string()
                    }));
                    print_output(&output)?;
                }
            }
            Ok(())
        }
        TxCommands::Simulate { .. } => {
            anyhow::bail!("command 'tx simulate' not yet implemented")
        }
        TxCommands::Sign { .. } => {
            anyhow::bail!("command 'tx sign' not yet implemented")
        }
        TxCommands::Submit { .. } => {
            anyhow::bail!("command 'tx submit' not yet implemented")
        }
    }
}
