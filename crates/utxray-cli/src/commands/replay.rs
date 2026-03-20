use clap::Subcommand;

use crate::context::AppContext;

#[derive(Subcommand, Debug)]
pub enum ReplayCommands {
    /// Bundle a failure for replay
    Bundle {
        #[arg(long)]
        from: Option<String>,
    },
    /// Run a replay bundle
    Run {
        #[arg(long)]
        bundle: Option<String>,
    },
    /// Diff two replay results
    Diff {
        #[arg(long)]
        before: Option<String>,
        #[arg(long)]
        after: Option<String>,
    },
}

pub async fn handle(_cmd: ReplayCommands, _ctx: &AppContext) -> anyhow::Result<()> {
    anyhow::bail!("command 'replay' not yet implemented")
}
