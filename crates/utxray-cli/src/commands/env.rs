use crate::context::AppContext;
use utxray_core::output::{print_output, Output};

pub async fn handle(ctx: &AppContext) -> anyhow::Result<()> {
    let config_loaded = true; // If we got here, config was loaded successfully

    let env_info = utxray_core::env::check_env(&ctx.config, &ctx.project, config_loaded).await?;

    let output = Output::ok(env_info);
    print_output(&output)?;
    Ok(())
}

pub async fn handle_gen_context(_ctx: &AppContext) -> anyhow::Result<()> {
    anyhow::bail!("command 'gen-context' not yet implemented")
}
