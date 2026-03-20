use crate::context::AppContext;

pub async fn handle(_module: Option<String>, _ctx: &AppContext) -> anyhow::Result<()> {
    anyhow::bail!("command 'typecheck' not yet implemented")
}
