use clap::{Args, Subcommand};
use utxray_core::cbor::decode::{decode_cbor_hex, DecodeErrorData};
use utxray_core::cbor::diff::{diff_cbor_hex, CborDiffErrorData};
use utxray_core::cbor::redeemer_index::{analyze_redeemer_index, RedeemerIndexErrorData};
use utxray_core::cbor::script_data_hash::{compute_script_data_hash, ScriptDataHashErrorData};
use utxray_core::output::{print_output_formatted, Output};

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

/// Resolve a CBOR input: if it looks like a file path, read its contents;
/// otherwise treat it as inline hex.
fn resolve_cbor_input(input: &str) -> String {
    let is_file = input.contains('/')
        || input.contains('\\')
        || input.ends_with(".hex")
        || input.ends_with(".cbor");
    if is_file {
        match std::fs::read_to_string(input) {
            Ok(contents) => contents.trim().to_string(),
            Err(_) => input.to_string(),
        }
    } else {
        input.to_string()
    }
}

pub async fn handle_cbor(cmd: CborCommands, ctx: &AppContext) -> anyhow::Result<()> {
    let format = &ctx.format;
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
                        print_output_formatted(&output, format)?;
                        return Ok(());
                    }
                }
            } else {
                let output = Output::error(DecodeErrorData {
                    error: "either --hex or --file must be provided".to_string(),
                });
                print_output_formatted(&output, format)?;
                return Ok(());
            };

            match decode_cbor_hex(&hex_input) {
                Ok(output) => {
                    print_output_formatted(&output, format)?;
                }
                Err(e) => {
                    let output = Output::error(DecodeErrorData {
                        error: e.to_string(),
                    });
                    print_output_formatted(&output, format)?;
                }
            }
            Ok(())
        }
        CborCommands::Diff { left, right } => {
            let left_input = match left {
                Some(l) => l,
                None => {
                    let output = Output::error(CborDiffErrorData {
                        error: "--left is required (hex string or file path)".to_string(),
                    });
                    print_output_formatted(&output, format)?;
                    return Ok(());
                }
            };
            let right_input = match right {
                Some(r) => r,
                None => {
                    let output = Output::error(CborDiffErrorData {
                        error: "--right is required (hex string or file path)".to_string(),
                    });
                    print_output_formatted(&output, format)?;
                    return Ok(());
                }
            };

            // Resolve inputs: could be hex strings or file paths
            let left_hex = resolve_cbor_input(&left_input);
            let right_hex = resolve_cbor_input(&right_input);

            match diff_cbor_hex(&left_hex, &right_hex) {
                Ok(output) => {
                    print_output_formatted(&output, format)?;
                }
                Err(e) => {
                    let output = Output::error(CborDiffErrorData {
                        error: e.to_string(),
                    });
                    print_output_formatted(&output, format)?;
                }
            }
            Ok(())
        }
    }
}

pub async fn handle_script_data_hash(
    args: ScriptDataHashArgs,
    ctx: &AppContext,
) -> anyhow::Result<()> {
    let format = &ctx.format;
    let redeemers = match &args.redeemers {
        Some(r) => r.as_str(),
        None => {
            let output = Output::error(ScriptDataHashErrorData {
                error: "missing required argument: --redeemers".to_string(),
            });
            print_output_formatted(&output, format)?;
            return Ok(());
        }
    };
    let datums = match &args.datums {
        Some(d) => d.as_str(),
        None => {
            let output = Output::error(ScriptDataHashErrorData {
                error: "missing required argument: --datums".to_string(),
            });
            print_output_formatted(&output, format)?;
            return Ok(());
        }
    };
    let cost_models = match &args.cost_models {
        Some(c) => c.as_str(),
        None => {
            let output = Output::error(ScriptDataHashErrorData {
                error: "missing required argument: --cost-models".to_string(),
            });
            print_output_formatted(&output, format)?;
            return Ok(());
        }
    };

    match compute_script_data_hash(redeemers, datums, cost_models) {
        Ok(output) => {
            print_output_formatted(&output, format)?;
        }
        Err(e) => {
            let output = Output::error(ScriptDataHashErrorData {
                error: e.to_string(),
            });
            print_output_formatted(&output, format)?;
        }
    }
    Ok(())
}

pub async fn handle_redeemer_index(
    args: RedeemerIndexArgs,
    ctx: &AppContext,
) -> anyhow::Result<()> {
    let format = &ctx.format;
    let tx_input = match &args.tx {
        Some(t) => t.as_str(),
        None => {
            let output = Output::error(RedeemerIndexErrorData {
                error: "missing required argument: --tx (hex string or file path)".to_string(),
            });
            print_output_formatted(&output, format)?;
            return Ok(());
        }
    };

    match analyze_redeemer_index(tx_input) {
        Ok(output) => {
            print_output_formatted(&output, format)?;
        }
        Err(e) => {
            let output = Output::error(RedeemerIndexErrorData {
                error: e.to_string(),
            });
            print_output_formatted(&output, format)?;
        }
    }
    Ok(())
}
