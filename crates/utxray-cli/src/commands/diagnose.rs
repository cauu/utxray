use clap::Args;

use crate::context::AppContext;

#[derive(Args, Debug)]
pub struct DiagnoseArgs {
    #[arg(long)]
    pub from: Option<String>,
    #[arg(long)]
    pub tx: Option<String>,
}

pub async fn handle(_args: DiagnoseArgs, _ctx: &AppContext) -> anyhow::Result<()> {
    anyhow::bail!("command 'diagnose' not yet implemented")
}
