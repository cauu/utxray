use clap::Subcommand;

use crate::context::AppContext;
use utxray_core::output::print_output;
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

pub async fn handle(cmd: TxCommands, ctx: &AppContext) -> anyhow::Result<()> {
    match cmd {
        TxCommands::Build { spec, exec_units } => {
            let spec_path = match spec {
                Some(p) => p,
                None => {
                    let output = utxray_core::output::Output::error(serde_json::json!({
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
        TxCommands::Evaluate { .. } => {
            anyhow::bail!("command 'tx evaluate' not yet implemented")
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
