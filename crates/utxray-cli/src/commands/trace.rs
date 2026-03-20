use clap::Args;

use crate::context::AppContext;
use utxray_core::output::print_output;
use utxray_core::trace::{run_trace, TraceConfig};

#[derive(Args, Debug)]
pub struct TraceArgs {
    /// Validator name (e.g. "escrow.spend" or full "escrow.escrow.spend")
    #[arg(long)]
    pub validator: String,

    /// Validator purpose: spend, mint, withdraw, publish
    #[arg(long)]
    pub purpose: String,

    /// Redeemer as inline JSON or path to JSON file
    #[arg(long)]
    pub redeemer: String,

    /// Datum as inline JSON or path to JSON file (required for spend)
    #[arg(long)]
    pub datum: Option<String>,

    /// Full script context as inline JSON or path to JSON file
    #[arg(long)]
    pub context: Option<String>,

    /// Current slot number for validity range checks
    #[arg(long)]
    pub slot: Option<u64>,

    /// Comma-separated signatory verification key hashes (56 hex chars each)
    #[arg(long, value_delimiter = ',')]
    pub signatories: Option<Vec<String>>,
}

pub async fn handle(args: TraceArgs, ctx: &AppContext) -> anyhow::Result<()> {
    let config = TraceConfig {
        validator: args.validator,
        purpose: args.purpose,
        redeemer: args.redeemer,
        datum: args.datum,
        context: args.context,
        slot: args.slot,
        signatories: args.signatories.unwrap_or_default(),
    };

    let output = run_trace(&ctx.project, config).await?;
    print_output(&output)?;
    Ok(())
}
