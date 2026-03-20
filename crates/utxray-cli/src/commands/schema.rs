use clap::Subcommand;

use crate::context::AppContext;

#[derive(Subcommand, Debug)]
pub enum SchemaCommands {
    /// Validate datum/redeemer against blueprint schema
    Validate {
        #[arg(long)]
        validator: Option<String>,
        #[arg(long)]
        purpose: Option<String>,
        #[arg(long)]
        datum: Option<String>,
        #[arg(long)]
        redeemer: Option<String>,
    },
}

pub async fn handle(_cmd: SchemaCommands, _ctx: &AppContext) -> anyhow::Result<()> {
    anyhow::bail!("command 'schema' not yet implemented")
}
