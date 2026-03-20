use clap::Subcommand;

use crate::context::AppContext;

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

pub async fn handle(_cmd: BudgetCommands, _ctx: &AppContext) -> anyhow::Result<()> {
    anyhow::bail!("command 'budget' not yet implemented")
}
