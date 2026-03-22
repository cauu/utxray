use clap::Subcommand;
use serde::Serialize;

use crate::context::AppContext;
use utxray_core::backend::blockfrost::BlockfrostBackend;
use utxray_core::backend::{DatumInfo, UtxoInfo};
use utxray_core::chain::utxo_diff;
use utxray_core::output::{print_output_formatted, Output};

#[derive(Subcommand, Debug)]
pub enum UtxoCommands {
    /// Query UTxOs at an address
    Query {
        #[arg(long)]
        address: Option<String>,
    },
    /// Diff UTxO sets before/after a transaction or slot range
    Diff {
        #[arg(long)]
        address: Option<String>,
        #[arg(long)]
        before_tx: Option<String>,
        #[arg(long)]
        after_tx: Option<String>,
        #[arg(long)]
        before_slot: Option<u64>,
        #[arg(long)]
        after_slot: Option<u64>,
    },
}

#[derive(Subcommand, Debug)]
pub enum DatumCommands {
    /// Resolve a datum by hash
    Resolve {
        #[arg(long)]
        hash: Option<String>,
    },
}

#[derive(Debug, Serialize)]
struct UtxoQueryOutput {
    utxos: Vec<UtxoInfo>,
}

#[derive(Debug, Serialize)]
struct DatumResolveOutput {
    hash: String,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    decoded: Option<serde_json::Value>,
}

fn get_blockfrost(ctx: &AppContext) -> anyhow::Result<BlockfrostBackend> {
    let project_id =
        ctx.config.blockfrost.project_id.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Blockfrost project_id not configured in .utxray.toml")
        })?;
    BlockfrostBackend::new(project_id, &ctx.network)
}

pub async fn handle_utxo(cmd: UtxoCommands, ctx: &AppContext) -> anyhow::Result<()> {
    let format = &ctx.format;
    match cmd {
        UtxoCommands::Query { address } => {
            let address = match address {
                Some(a) => a,
                None => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "MISSING_ARGUMENT",
                        "message": "--address is required"
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

            match backend.query_utxos(&address).await {
                Ok(utxos) => {
                    let output = Output::ok(UtxoQueryOutput { utxos });
                    print_output_formatted(&output, format)?;
                }
                Err(e) => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "QUERY_FAILED",
                        "message": e.to_string()
                    }));
                    print_output_formatted(&output, format)?;
                }
            }
            Ok(())
        }
        UtxoCommands::Diff {
            address,
            before_tx,
            after_tx,
            before_slot,
            after_slot,
        } => {
            let address = match address {
                Some(a) => a,
                None => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "MISSING_ARGUMENT",
                        "message": "--address is required"
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

            // Determine mode: by_tx or by_slot
            if let (Some(bt), Some(at)) = (before_tx, after_tx) {
                match utxo_diff::diff_by_tx(&address, &bt, &at, &backend).await {
                    Ok(output) => {
                        print_output_formatted(&output, format)?;
                    }
                    Err(e) => {
                        let output = Output::error(serde_json::json!({
                            "error_code": "UTXO_DIFF_FAILED",
                            "message": e.to_string()
                        }));
                        print_output_formatted(&output, format)?;
                    }
                }
            } else if let (Some(bs), Some(a_s)) = (before_slot, after_slot) {
                match utxo_diff::diff_by_slot(&address, bs, a_s, &backend).await {
                    Ok(output) => {
                        print_output_formatted(&output, format)?;
                    }
                    Err(e) => {
                        let output = Output::error(serde_json::json!({
                            "error_code": "UTXO_DIFF_FAILED",
                            "message": e.to_string()
                        }));
                        print_output_formatted(&output, format)?;
                    }
                }
            } else {
                let output = Output::error(serde_json::json!({
                    "error_code": "MISSING_ARGUMENT",
                    "message": "either (--before-tx + --after-tx) or (--before-slot + --after-slot) is required"
                }));
                print_output_formatted(&output, format)?;
            }
            Ok(())
        }
    }
}

pub async fn handle_datum(cmd: DatumCommands, ctx: &AppContext) -> anyhow::Result<()> {
    let format = &ctx.format;
    match cmd {
        DatumCommands::Resolve { hash } => {
            let hash = match hash {
                Some(h) => h,
                None => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "MISSING_ARGUMENT",
                        "message": "--hash is required"
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

            match backend.resolve_datum(&hash).await {
                Ok(Some(DatumInfo {
                    hash: h,
                    source,
                    decoded,
                })) => {
                    let output = Output::ok(DatumResolveOutput {
                        hash: h,
                        source,
                        decoded: Some(decoded),
                    });
                    print_output_formatted(&output, format)?;
                }
                Ok(None) => {
                    // Not found -> status ok, source "unresolved"
                    let output = Output::ok(DatumResolveOutput {
                        hash,
                        source: "unresolved".to_string(),
                        decoded: None,
                    });
                    print_output_formatted(&output, format)?;
                }
                Err(e) => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "RESOLVE_FAILED",
                        "message": e.to_string()
                    }));
                    print_output_formatted(&output, format)?;
                }
            }
            Ok(())
        }
    }
}
