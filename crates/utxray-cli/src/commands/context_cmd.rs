use clap::Subcommand;
use serde::Serialize;

use crate::context::AppContext;
use utxray_core::backend::blockfrost::BlockfrostBackend;
use utxray_core::backend::TipInfo;
use utxray_core::output::{print_output, Output};

#[derive(Subcommand, Debug)]
pub enum ContextCommands {
    /// Query protocol parameters
    Params,
    /// Query current tip (slot, time)
    Tip,
}

#[derive(Debug, Serialize)]
struct ParamsOutput {
    params: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct TipOutput {
    tip: TipInfo,
}

fn get_blockfrost(ctx: &AppContext) -> anyhow::Result<BlockfrostBackend> {
    let project_id =
        ctx.config.blockfrost.project_id.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Blockfrost project_id not configured in .utxray.toml")
        })?;
    BlockfrostBackend::new(project_id, &ctx.network)
}

pub async fn handle(cmd: ContextCommands, ctx: &AppContext) -> anyhow::Result<()> {
    match cmd {
        ContextCommands::Params => {
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

            match backend.query_params().await {
                Ok(params) => {
                    let output = Output::ok(ParamsOutput { params });
                    print_output(&output)?;
                }
                Err(e) => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "QUERY_FAILED",
                        "message": e.to_string()
                    }));
                    print_output(&output)?;
                }
            }
            Ok(())
        }
        ContextCommands::Tip => {
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

            match backend.query_tip().await {
                Ok(tip) => {
                    let output = Output::ok(TipOutput { tip });
                    print_output(&output)?;
                }
                Err(e) => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "QUERY_FAILED",
                        "message": e.to_string()
                    }));
                    print_output(&output)?;
                }
            }
            Ok(())
        }
    }
}
