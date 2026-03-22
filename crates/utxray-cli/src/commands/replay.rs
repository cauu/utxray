use clap::Subcommand;

use utxray_core::output::{print_output_formatted, Output};
use utxray_core::replay::{bundle, runner};

use crate::context::AppContext;

#[derive(Subcommand, Debug)]
pub enum ReplayCommands {
    /// Bundle a failure for replay
    Bundle {
        /// Path to result JSON file
        #[arg(long)]
        from: Option<String>,
        /// Optional path to transaction CBOR file
        #[arg(long)]
        tx: Option<String>,
        /// Output file path (default: replay.bundle.json)
        #[arg(long)]
        output: Option<String>,
    },
    /// Run a replay bundle
    Run {
        /// Path to bundle file
        #[arg(long)]
        bundle: Option<String>,
    },
    /// Diff two replay results
    Diff {
        #[arg(long)]
        before: Option<String>,
        #[arg(long)]
        after: Option<String>,
    },
}

pub async fn handle(cmd: ReplayCommands, ctx: &AppContext) -> anyhow::Result<()> {
    let format = &ctx.format;
    match cmd {
        ReplayCommands::Bundle { from, tx, output } => {
            let result = bundle::create_bundle(
                from.as_deref(),
                tx.as_deref(),
                output.as_deref(),
                &ctx.project,
                &ctx.network,
            )
            .await?;
            print_output_formatted(&result, format)?;
        }
        ReplayCommands::Run {
            bundle: bundle_path,
        } => {
            let result = runner::run_bundle(bundle_path.as_deref(), &ctx.project).await?;
            print_output_formatted(&result, format)?;
        }
        ReplayCommands::Diff { .. } => {
            let output = Output::error(serde_json::json!({
                "error_code": "NOT_IMPLEMENTED",
                "message": "command 'replay diff' is not yet implemented"
            }));
            print_output_formatted(&output, format)?;
        }
    }
    Ok(())
}
