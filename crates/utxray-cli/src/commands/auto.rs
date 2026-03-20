use clap::Args;

use crate::context::AppContext;

#[derive(Args, Debug)]
pub struct AutoArgs {
    #[arg(long)]
    pub validator: Option<String>,
    #[arg(long)]
    pub purpose: Option<String>,
    #[arg(long, default_value = "full")]
    pub scenario: String,
    #[arg(long)]
    pub datum: Option<String>,
    #[arg(long)]
    pub redeemer: Option<String>,
    #[arg(long)]
    pub tx_spec: Option<String>,
}

pub async fn handle(_args: AutoArgs, _ctx: &AppContext) -> anyhow::Result<()> {
    anyhow::bail!("command 'auto' not yet implemented")
}
