use clap::Subcommand;

use crate::context::AppContext;

#[derive(Subcommand, Debug)]
pub enum ContextCommands {
    /// Query protocol parameters
    Params,
    /// Query current tip (slot, time)
    Tip,
}

pub async fn handle(_cmd: ContextCommands, _ctx: &AppContext) -> anyhow::Result<()> {
    anyhow::bail!("command 'context' not yet implemented")
}
