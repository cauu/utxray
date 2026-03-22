use clap::Subcommand;

use utxray_core::output::print_output_formatted;

use crate::context::AppContext;

#[derive(Subcommand, Debug)]
pub enum UplcCommands {
    /// Evaluate a UPLC program
    Eval {
        /// Path to the .uplc file
        file: String,
        /// Arguments as JSON
        #[arg(long)]
        args: Option<String>,
    },
}

pub async fn handle(cmd: UplcCommands, ctx: &AppContext) -> anyhow::Result<()> {
    match cmd {
        UplcCommands::Eval { file, args } => {
            let output =
                utxray_core::uplc::eval(&file, args.as_deref(), &ctx.project, ctx.verbose).await?;
            print_output_formatted(&output, &ctx.format)?;
        }
    }
    Ok(())
}
