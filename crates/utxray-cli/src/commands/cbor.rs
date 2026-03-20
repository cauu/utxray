use clap::{Args, Subcommand};
use utxray_core::cbor::decode::{decode_cbor_hex, DecodeErrorData};
use utxray_core::output::{print_output, Output};

use crate::context::AppContext;

#[derive(Subcommand, Debug)]
pub enum CborCommands {
    /// Decode CBOR hex to human-readable JSON
    Decode {
        #[arg(long)]
        hex: Option<String>,
        #[arg(long)]
        file: Option<String>,
        #[arg(long)]
        r#type: Option<String>,
    },
    /// Diff two CBOR values
    Diff {
        #[arg(long)]
        left: Option<String>,
        #[arg(long)]
        right: Option<String>,
    },
}

#[derive(Args, Debug)]
pub struct ScriptDataHashArgs {
    #[arg(long)]
    pub redeemers: Option<String>,
    #[arg(long)]
    pub datums: Option<String>,
    #[arg(long)]
    pub cost_models: Option<String>,
}

#[derive(Args, Debug)]
pub struct RedeemerIndexArgs {
    #[arg(long)]
    pub tx: Option<String>,
    #[arg(long)]
    pub purpose: Option<String>,
}

pub async fn handle_cbor(cmd: CborCommands, _ctx: &AppContext) -> anyhow::Result<()> {
    match cmd {
        CborCommands::Decode {
            hex,
            file,
            r#type: _type,
        } => {
            let hex_input = if let Some(h) = hex {
                h
            } else if let Some(f) = file {
                match std::fs::read_to_string(&f) {
                    Ok(contents) => contents.trim().to_string(),
                    Err(e) => {
                        let output = Output::error(DecodeErrorData {
                            error: format!("failed to read file '{f}': {e}"),
                        });
                        print_output(&output)?;
                        return Ok(());
                    }
                }
            } else {
                let output = Output::error(DecodeErrorData {
                    error: "either --hex or --file must be provided".to_string(),
                });
                print_output(&output)?;
                return Ok(());
            };

            match decode_cbor_hex(&hex_input) {
                Ok(output) => {
                    print_output(&output)?;
                }
                Err(e) => {
                    let output = Output::error(DecodeErrorData {
                        error: e.to_string(),
                    });
                    print_output(&output)?;
                }
            }
            Ok(())
        }
        CborCommands::Diff { .. } => {
            anyhow::bail!("command 'cbor diff' not yet implemented")
        }
    }
}

pub async fn handle_script_data_hash(
    _args: ScriptDataHashArgs,
    _ctx: &AppContext,
) -> anyhow::Result<()> {
    anyhow::bail!("command 'script-data-hash' not yet implemented")
}

pub async fn handle_redeemer_index(
    _args: RedeemerIndexArgs,
    _ctx: &AppContext,
) -> anyhow::Result<()> {
    anyhow::bail!("command 'redeemer-index' not yet implemented")
}
