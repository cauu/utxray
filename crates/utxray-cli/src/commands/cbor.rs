use clap::{Args, Subcommand};

use crate::context::AppContext;

#[derive(Subcommand, Debug)]
pub enum CborCommands {
    /// Decode CBOR hex to human-readable JSON
    Decode {
        #[arg(long)]
        hex: Option<String>,
        #[arg(long)]
        file: Option<String>,
        #[arg(long)]
        r#type: Option<String>,
    },
    /// Diff two CBOR values
    Diff {
        #[arg(long)]
        left: Option<String>,
        #[arg(long)]
        right: Option<String>,
    },
}

#[derive(Args, Debug)]
pub struct ScriptDataHashArgs {
    #[arg(long)]
    pub redeemers: Option<String>,
    #[arg(long)]
    pub datums: Option<String>,
    #[arg(long)]
    pub cost_models: Option<String>,
}

#[derive(Args, Debug)]
pub struct RedeemerIndexArgs {
    #[arg(long)]
    pub tx: Option<String>,
    #[arg(long)]
    pub purpose: Option<String>,
}

pub async fn handle_cbor(_cmd: CborCommands, _ctx: &AppContext) -> anyhow::Result<()> {
    anyhow::bail!("command 'cbor' not yet implemented")
}

pub async fn handle_script_data_hash(
    _args: ScriptDataHashArgs,
    _ctx: &AppContext,
) -> anyhow::Result<()> {
    anyhow::bail!("command 'script-data-hash' not yet implemented")
}

pub async fn handle_redeemer_index(
    _args: RedeemerIndexArgs,
    _ctx: &AppContext,
) -> anyhow::Result<()> {
    anyhow::bail!("command 'redeemer-index' not yet implemented")
}
