use crate::context::AppContext;
use utxray_core::output::{print_output_formatted, Output};

pub async fn handle(ctx: &AppContext) -> anyhow::Result<()> {
    let config_loaded = true; // If we got here, config was loaded successfully

    let env_info =
        utxray_core::env::check_env(&ctx.config, &ctx.project, config_loaded, &ctx.network).await?;

    let output = Output::ok(env_info);
    print_output_formatted(&output, &ctx.format)?;
    Ok(())
}

pub async fn handle_gen_context(ctx: &AppContext) -> anyhow::Result<()> {
    let output = Output::error(serde_json::json!({
        "error_code": "NOT_IMPLEMENTED",
        "message": "command 'gen-context' is not yet implemented"
    }));
    print_output_formatted(&output, &ctx.format)?;
    Ok(())
}
