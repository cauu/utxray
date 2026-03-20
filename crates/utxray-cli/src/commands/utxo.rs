use clap::Subcommand;

use crate::context::AppContext;

#[derive(Subcommand, Debug)]
pub enum UtxoCommands {
    /// Query UTxOs at an address
    Query {
        #[arg(long)]
        address: Option<String>,
    },
    /// Diff UTxO sets before/after a transaction
    Diff {
        #[arg(long)]
        before: Option<String>,
        #[arg(long)]
        after: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum DatumCommands {
    /// Resolve a datum by hash
    Resolve {
        #[arg(long)]
        hash: Option<String>,
    },
}

pub async fn handle_utxo(_cmd: UtxoCommands, _ctx: &AppContext) -> anyhow::Result<()> {
    anyhow::bail!("command 'utxo' not yet implemented")
}

pub async fn handle_datum(_cmd: DatumCommands, _ctx: &AppContext) -> anyhow::Result<()> {
    anyhow::bail!("command 'datum' not yet implemented")
}
