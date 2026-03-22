use clap::Subcommand;

use crate::context::AppContext;
use utxray_core::output::{print_output_formatted, Output};

#[derive(Subcommand, Debug)]
pub enum BlueprintCommands {
    /// Show blueprint info (default)
    Show {
        /// Path to blueprint file (defaults to plutus.json in project dir)
        #[arg(long)]
        file: Option<String>,
    },
    /// Apply parameters to a parameterized validator
    Apply {
        /// Validator name or index
        #[arg(long)]
        validator: Option<String>,
        /// Parameters as JSON
        #[arg(long)]
        params: Option<String>,
        /// Path to blueprint file
        #[arg(long)]
        file: Option<String>,
    },
    /// Convert blueprint to cardano-cli compatible format
    Convert {
        /// Validator name or index
        #[arg(long)]
        validator: Option<String>,
        /// Output file path
        #[arg(long)]
        out: Option<String>,
        /// Path to blueprint file
        #[arg(long)]
        file: Option<String>,
    },
}

pub async fn handle_build(_watch: bool, ctx: &AppContext) -> anyhow::Result<()> {
    let output = utxray_core::build::run_build(&ctx.project).await?;
    print_output_formatted(&output, &ctx.format)?;
    Ok(())
}

pub async fn handle_blueprint(cmd: BlueprintCommands, ctx: &AppContext) -> anyhow::Result<()> {
    match cmd {
        BlueprintCommands::Show { file } => {
            match utxray_core::blueprint::blueprint_show(&ctx.project, file.as_deref()) {
                Ok(output) => print_output_formatted(&output, &ctx.format)?,
                Err(e) => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "BLUEPRINT_ERROR",
                        "message": e.to_string(),
                    }));
                    print_output_formatted(&output, &ctx.format)?;
                }
            }
        }
        BlueprintCommands::Apply {
            validator,
            params,
            file,
        } => {
            match utxray_core::blueprint::blueprint_apply(
                &ctx.project,
                file.as_deref(),
                validator.as_deref(),
                params.as_deref(),
            ) {
                Ok(output) => print_output_formatted(&output, &ctx.format)?,
                Err(e) => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "BLUEPRINT_ERROR",
                        "message": e.to_string(),
                    }));
                    print_output_formatted(&output, &ctx.format)?;
                }
            }
        }
        BlueprintCommands::Convert {
            validator,
            out,
            file,
        } => {
            match utxray_core::blueprint::blueprint_convert(
                &ctx.project,
                file.as_deref(),
                validator.as_deref(),
                out.as_deref(),
            ) {
                Ok(output) => print_output_formatted(&output, &ctx.format)?,
                Err(e) => {
                    let output = Output::error(serde_json::json!({
                        "error_code": "BLUEPRINT_ERROR",
                        "message": e.to_string(),
                    }));
                    print_output_formatted(&output, &ctx.format)?;
                }
            }
        }
    }
    Ok(())
}
