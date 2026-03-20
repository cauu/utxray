use crate::context::AppContext;
use utxray_core::output::print_output;

pub async fn handle(module: Option<String>, ctx: &AppContext) -> anyhow::Result<()> {
    let trace_level = &ctx.config.defaults.trace_level;
    let output =
        utxray_core::typecheck::run_typecheck(&ctx.project, module.as_deref(), trace_level).await?;
    print_output(&output)?;
    Ok(())
}
