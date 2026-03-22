use clap::Args;

use utxray_core::output::print_output_formatted;

use crate::context::AppContext;

#[derive(Args, Debug)]
pub struct TestSequenceArgs {
    /// Path to the sequence spec JSON file
    #[arg(long)]
    pub spec: String,
}

pub async fn handle(args: TestSequenceArgs, ctx: &AppContext) -> anyhow::Result<()> {
    let output = utxray_core::test_sequence::run_sequence(&args.spec, &ctx.project).await?;
    print_output_formatted(&output, &ctx.format)?;
    Ok(())
}
