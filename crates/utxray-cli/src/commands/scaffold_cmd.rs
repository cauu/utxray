use clap::Subcommand;

use utxray_core::output::{print_output_formatted, Output};

use crate::context::AppContext;

#[derive(Subcommand, Debug)]
pub enum ScaffoldCommands {
    /// Generate a minimal test stub for a validator
    Test {
        /// Validator name or index
        #[arg(long)]
        validator: String,
        /// Path to blueprint file (defaults to plutus.json)
        #[arg(long)]
        file: Option<String>,
        /// Write the generated file (default: print to stdout only)
        #[arg(long)]
        write: bool,
    },
}

pub async fn handle(cmd: ScaffoldCommands, ctx: &AppContext) -> anyhow::Result<()> {
    match cmd {
        ScaffoldCommands::Test {
            validator,
            file,
            write,
        } => {
            match utxray_core::scaffold::scaffold_test(
                &ctx.project,
                &validator,
                file.as_deref(),
                write,
            ) {
                Ok(output) => print_output_formatted(&output, &ctx.format)?,
                Err(e) => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "SCAFFOLD_ERROR",
                        "message": e.to_string(),
                    }));
                    print_output_formatted(&output, &ctx.format)?;
                }
            }
        }
    }
    Ok(())
}
