use clap::Subcommand;

use crate::context::AppContext;
use utxray_core::budget;
use utxray_core::output::print_output_formatted;

#[derive(Subcommand, Debug)]
pub enum BudgetCommands {
    /// Show budget analysis (default)
    Show {
        /// Filter to a specific validator by name
        #[arg(long)]
        validator: Option<String>,
        /// Show all validators (default behavior)
        #[arg(long, default_value_t = false)]
        all: bool,
    },
    /// Compare budgets between two result files
    Compare {
        /// Path to the "before" result JSON file
        #[arg(long)]
        before: Option<String>,
        /// Path to the "after" result JSON file
        #[arg(long)]
        after: Option<String>,
        /// Filter to a specific validator by name
        #[arg(long)]
        validator: Option<String>,
    },
}

pub async fn handle(cmd: BudgetCommands, ctx: &AppContext) -> anyhow::Result<()> {
    let format = &ctx.format;
    match cmd {
        BudgetCommands::Show { validator, .. } => {
            let result = budget::budget_show(&ctx.project, validator.as_deref()).await?;
            print_output_formatted(&result, format)?;
        }
        BudgetCommands::Compare {
            before,
            after,
            validator,
        } => {
            let result =
                budget::budget_compare(before.as_deref(), after.as_deref(), validator.as_deref())?;
            print_output_formatted(&result, format)?;
        }
    }
    Ok(())
}
