use clap::Args;

use crate::context::AppContext;

#[derive(Args, Debug)]
pub struct TraceArgs {
    #[arg(long)]
    pub validator: Option<String>,
    #[arg(long)]
    pub purpose: Option<String>,
    #[arg(long)]
    pub datum: Option<String>,
    #[arg(long)]
    pub redeemer: Option<String>,
}

pub async fn handle(_args: TraceArgs, _ctx: &AppContext) -> anyhow::Result<()> {
    anyhow::bail!("command 'trace' not yet implemented")
}
