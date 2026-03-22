use clap::Args;

use utxray_core::diagnose;
use utxray_core::output::print_output_formatted;

use crate::context::AppContext;

#[derive(Args, Debug)]
pub struct DiagnoseArgs {
    /// Path to result JSON file, or "-" for stdin
    #[arg(long)]
    pub from: Option<String>,
    /// Optional path to transaction CBOR file
    #[arg(long)]
    pub tx: Option<String>,
}

pub async fn handle(args: DiagnoseArgs, ctx: &AppContext) -> anyhow::Result<()> {
    let output = diagnose::run_diagnose(args.from.as_deref()).await?;
    print_output_formatted(&output, &ctx.format)?;
    Ok(())
}
