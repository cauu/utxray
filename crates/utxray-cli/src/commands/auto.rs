use clap::Args;

use crate::context::AppContext;
use utxray_core::output::{print_output_formatted, Output};

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

pub async fn handle(_args: AutoArgs, ctx: &AppContext) -> anyhow::Result<()> {
    let output = Output::error(serde_json::json!({
        "error_code": "NOT_IMPLEMENTED",
        "message": "command 'auto' is not yet implemented"
    }));
    print_output_formatted(&output, &ctx.format)?;
    Ok(())
}
