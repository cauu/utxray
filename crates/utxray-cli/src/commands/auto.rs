use clap::Args;

use crate::context::AppContext;
use utxray_core::output::print_output_formatted;

#[derive(Args, Debug)]
pub struct AutoArgs {
    #[arg(long)]
    pub validator: Option<String>,
    #[arg(long)]
    pub purpose: Option<String>,
    #[arg(long, default_value = "full")]
    pub scenario: String,
    #[arg(long)]
    pub datum: Option<String>,
    #[arg(long)]
    pub redeemer: Option<String>,
    #[arg(long)]
    pub tx_spec: Option<String>,
}

pub async fn handle(args: AutoArgs, ctx: &AppContext) -> anyhow::Result<()> {
    let params = utxray_core::auto::AutoParams {
        project_dir: &ctx.project,
        scenario: &args.scenario,
        validator: args.validator.as_deref(),
        purpose: args.purpose.as_deref(),
        datum: args.datum.as_deref(),
        redeemer: args.redeemer.as_deref(),
        tx_spec: args.tx_spec.as_deref(),
    };

    let output = utxray_core::auto::run_auto(params).await?;
    print_output_formatted(&output, &ctx.format)?;
    Ok(())
}
