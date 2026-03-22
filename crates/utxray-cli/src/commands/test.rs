use clap::Args;

use crate::context::AppContext;
use utxray_core::output::print_output_formatted;

#[derive(Args, Debug)]
pub struct TestArgs {
    #[arg(long)]
    pub module: Option<String>,
    #[arg(long)]
    pub r#match: Option<String>,
    #[arg(long, default_value = "verbose")]
    pub trace_level: String,
    #[arg(long)]
    pub seed: Option<u64>,
}

pub async fn handle(args: TestArgs, ctx: &AppContext) -> anyhow::Result<()> {
    let output = utxray_core::test_cmd::run_test(
        &ctx.project,
        args.r#match.as_deref(),
        args.module.as_deref(),
        &args.trace_level,
        args.seed,
    )
    .await?;
    print_output_formatted(&output, &ctx.format)?;
    Ok(())
}
