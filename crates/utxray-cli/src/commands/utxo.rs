use clap::Subcommand;
use serde::Serialize;

use crate::context::AppContext;
use utxray_core::backend::blockfrost::BlockfrostBackend;
use utxray_core::backend::{DatumInfo, UtxoInfo};
use utxray_core::output::{print_output_formatted, Output};

#[derive(Subcommand, Debug)]
pub enum UtxoCommands {
    /// Query UTxOs at an address
    Query {
        #[arg(long)]
        address: Option<String>,
    },
    /// Diff UTxO sets before/after a transaction
    Diff {
        #[arg(long)]
        before: Option<String>,
        #[arg(long)]
        after: Option<String>,
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
        UtxoCommands::Diff { .. } => {
            let output = Output::error(serde_json::json!({
                "error_code": "NOT_IMPLEMENTED",
                "message": "command 'utxo diff' is not yet implemented"
            }));
            print_output_formatted(&output, format)?;
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
