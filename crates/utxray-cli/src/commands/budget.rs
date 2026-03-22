use clap::Subcommand;

use crate::context::AppContext;
use utxray_core::output::{print_output_formatted, Output};

#[derive(Subcommand, Debug)]
pub enum BudgetCommands {
    /// Show budget analysis (default)
    Show {
        #[arg(long)]
        tx: Option<String>,
    },
    /// Compare budgets between two runs
    Compare {
        #[arg(long)]
        before: Option<String>,
        #[arg(long)]
        after: Option<String>,
    },
}

pub async fn handle(_cmd: BudgetCommands, ctx: &AppContext) -> anyhow::Result<()> {
    let output = Output::error(serde_json::json!({
        "error_code": "NOT_IMPLEMENTED",
        "message": "command 'budget' is not yet implemented"
    }));
    print_output_formatted(&output, &ctx.format)?;
    Ok(())
}
