use clap::Subcommand;
use serde::Serialize;

use crate::context::AppContext;
use utxray_core::backend::blockfrost::BlockfrostBackend;
use utxray_core::backend::EvaluatedRedeemer;
use utxray_core::output::{print_output_formatted, Output};
use utxray_core::tx::builder;
use utxray_core::tx::signer;
use utxray_core::tx::simulator;
use utxray_core::tx::submitter;

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
        #[arg(long)]
        backend: Option<String>,
        #[arg(long)]
        additional_utxo: Option<String>,
        #[arg(long)]
        slot: Option<u64>,
    },
    /// Sign a transaction
    Sign {
        #[arg(long)]
        tx: Option<String>,
        #[arg(long)]
        signing_key: Option<String>,
        #[arg(long)]
        out: Option<String>,
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

#[derive(Debug, Serialize)]
struct TxSignOutput {
    is_signed: bool,
    tx_file: String,
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
    let format = &ctx.format;
    match cmd {
        TxCommands::Build { spec, exec_units } => {
            let spec_path = match spec {
                Some(p) => p,
                None => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "MISSING_ARGUMENT",
                        "message": "--spec <tx-spec.json> is required"
                    }));
                    print_output_formatted(&output, format)?;
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
                &ctx.network,
            );
            print_output_formatted(&output, format)?;
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
                    print_output_formatted(&output, format)?;
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
                    print_output_formatted(&output, format)?;
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
                    print_output_formatted(&output, format)?;
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
                    print_output_formatted(&output, format)?;
                }
                Err(e) => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "EVALUATE_FAILED",
                        "message": e.to_string()
                    }));
                    print_output_formatted(&output, format)?;
                }
            }
            Ok(())
        }
        TxCommands::Simulate {
            tx,
            backend: _backend_flag,
            additional_utxo: _additional_utxo,
            slot: _slot,
        } => {
            let tx_arg = match tx {
                Some(t) => t,
                None => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "MISSING_ARGUMENT",
                        "message": "--tx <cbor_hex_or_file> is required"
                    }));
                    print_output_formatted(&output, format)?;
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
                    print_output_formatted(&output, format)?;
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
                    print_output_formatted(&output, format)?;
                    return Ok(());
                }
            };

            match simulator::simulate_tx(&cbor_hex, &backend, "blockfrost").await {
                Ok(output) => {
                    print_output_formatted(&output, format)?;
                }
                Err(e) => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "SIMULATE_FAILED",
                        "message": e.to_string()
                    }));
                    print_output_formatted(&output, format)?;
                }
            }
            Ok(())
        }
        TxCommands::Sign {
            tx,
            signing_key,
            out,
        } => {
            let tx_arg = match tx {
                Some(t) => t,
                None => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "MISSING_ARGUMENT",
                        "message": "--tx <cbor_hex_or_file> is required"
                    }));
                    print_output_formatted(&output, format)?;
                    return Ok(());
                }
            };

            let skey_path = match signing_key {
                Some(s) => s,
                None => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "MISSING_ARGUMENT",
                        "message": "--signing-key <skey-file> is required"
                    }));
                    print_output_formatted(&output, format)?;
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
                    print_output_formatted(&output, format)?;
                    return Ok(());
                }
            };

            let tx_bytes = match hex::decode(&cbor_hex) {
                Ok(b) => b,
                Err(e) => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "INVALID_HEX",
                        "message": format!("invalid transaction hex: {e}")
                    }));
                    print_output_formatted(&output, format)?;
                    return Ok(());
                }
            };

            let signed_bytes = match signer::sign_transaction(&tx_bytes, &skey_path) {
                Ok(b) => b,
                Err(e) => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "SIGN_FAILED",
                        "message": e.to_string()
                    }));
                    print_output_formatted(&output, format)?;
                    return Ok(());
                }
            };

            let signed_hex = hex::encode(&signed_bytes);
            let out_path = out.unwrap_or_else(|| "./tx.signed".to_string());

            if let Err(e) = tokio::fs::write(&out_path, &signed_hex).await {
                let output = Output::error(serde_json::json!({
                    "error_code": "WRITE_FAILED",
                    "message": format!("failed to write signed tx to '{}': {}", out_path, e)
                }));
                print_output_formatted(&output, format)?;
                return Ok(());
            }

            let output = Output::ok(TxSignOutput {
                is_signed: true,
                tx_file: out_path,
            });
            print_output_formatted(&output, format)?;
            Ok(())
        }
        TxCommands::Submit { tx, allow_mainnet } => {
            let tx_arg = match tx {
                Some(t) => t,
                None => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "MISSING_ARGUMENT",
                        "message": "--tx <cbor_hex_or_file> is required"
                    }));
                    print_output_formatted(&output, format)?;
                    return Ok(());
                }
            };

            // Mainnet safety check before even reading the backend config
            if ctx.network == "mainnet" && !allow_mainnet {
                let output = Output::error(serde_json::json!({
                    "error_code": "MAINNET_SAFETY_BLOCK",
                    "severity": "critical",
                    "message": "Refusing to submit to mainnet without --allow-mainnet flag."
                }));
                print_output_formatted(&output, format)?;
                return Ok(());
            }

            let backend = match get_blockfrost(ctx) {
                Ok(b) => b,
                Err(e) => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "BACKEND_NOT_CONFIGURED",
                        "message": e.to_string()
                    }));
                    print_output_formatted(&output, format)?;
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
                    print_output_formatted(&output, format)?;
                    return Ok(());
                }
            };

            match submitter::submit_transaction(&cbor_hex, &ctx.network, allow_mainnet, &backend)
                .await
            {
                Ok(output) => {
                    print_output_formatted(&output, format)?;
                }
                Err(e) => {
                    let (error_code, severity) = match &e {
                        submitter::SubmitError::MainnetSafetyBlock => {
                            ("MAINNET_SAFETY_BLOCK", "critical")
                        }
                        _ => ("SUBMIT_FAILED", "error"),
                    };
                    let output = Output::error(serde_json::json!({
                        "error_code": error_code,
                        "severity": severity,
                        "message": e.to_string()
                    }));
                    print_output_formatted(&output, format)?;
                }
            }
            Ok(())
        }
    }
}
