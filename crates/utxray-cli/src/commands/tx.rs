use clap::Subcommand;

use crate::context::AppContext;

#[derive(Subcommand, Debug)]
pub enum TxCommands {
    /// Build a transaction
    Build {
        #[arg(long)]
        spec: Option<String>,
        #[arg(long)]
        exec_units: Option<String>,
    },
    /// Evaluate transaction ExUnits
    Evaluate {
        #[arg(long)]
        tx: Option<String>,
    },
    /// Simulate a transaction (full phase-1 + phase-2 validation)
    Simulate {
        #[arg(long)]
        tx: Option<String>,
    },
    /// Sign a transaction
    Sign {
        #[arg(long)]
        tx: Option<String>,
        #[arg(long)]
        signing_key: Option<String>,
    },
    /// Submit a transaction
    Submit {
        #[arg(long)]
        tx: Option<String>,
        #[arg(long)]
        allow_mainnet: bool,
    },
}

pub async fn handle(_cmd: TxCommands, _ctx: &AppContext) -> anyhow::Result<()> {
    anyhow::bail!("command 'tx' not yet implemented")
}
