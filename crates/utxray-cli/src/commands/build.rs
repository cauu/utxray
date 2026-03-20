use clap::Subcommand;

use crate::context::AppContext;

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

pub async fn handle_build(_watch: bool, _ctx: &AppContext) -> anyhow::Result<()> {
    anyhow::bail!("command 'build' not yet implemented")
}

pub async fn handle_blueprint(_cmd: BlueprintCommands, _ctx: &AppContext) -> anyhow::Result<()> {
    anyhow::bail!("command 'blueprint' not yet implemented")
}
