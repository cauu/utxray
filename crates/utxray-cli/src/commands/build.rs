use clap::Subcommand;

use crate::context::AppContext;
use utxray_core::output::{print_output_formatted, Output};

#[derive(Subcommand, Debug)]
pub enum BlueprintCommands {
    /// Show blueprint info (default)
    Show,
    /// Apply parameters to a parameterized validator
    Apply {
        #[arg(long)]
        validator: Option<String>,
        #[arg(long)]
        params: Option<String>,
    },
    /// Convert blueprint to cardano-cli compatible format
    Convert {
        #[arg(long)]
        output: Option<String>,
    },
}

pub async fn handle_build(_watch: bool, ctx: &AppContext) -> anyhow::Result<()> {
    let output = utxray_core::build::run_build(&ctx.project).await?;
    print_output_formatted(&output, &ctx.format)?;
    Ok(())
}

pub async fn handle_blueprint(_cmd: BlueprintCommands, ctx: &AppContext) -> anyhow::Result<()> {
    let output = Output::error(serde_json::json!({
        "error_code": "NOT_IMPLEMENTED",
        "message": "command 'blueprint' is not yet implemented"
    }));
    print_output_formatted(&output, &ctx.format)?;
    Ok(())
}
