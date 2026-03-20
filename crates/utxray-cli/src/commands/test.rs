use clap::Args;

use crate::context::AppContext;

#[derive(Args, Debug)]
pub struct TestArgs {
    #[arg(long)]
    pub module: Option<String>,
    #[arg(long)]
    pub r#match: Option<String>,
    #[arg(long, default_value = "verbose")]
    pub trace_level: String,
}

pub async fn handle(_args: TestArgs, _ctx: &AppContext) -> anyhow::Result<()> {
    anyhow::bail!("command 'test' not yet implemented")
}
