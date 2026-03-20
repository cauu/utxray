use utxray_core::config::Config;

/// Application context passed to all command handlers
#[allow(dead_code)]
pub struct AppContext {
    pub config: Config,
    pub format: String,
    pub project: String,
    pub network: String,
    pub include_raw: bool,
    pub verbose: bool,
    pub backend_override: Option<String>,
}

impl AppContext {
    pub fn new(
        project: &str,
        network: &str,
        format: &str,
        include_raw: bool,
        verbose: bool,
        backend: Option<String>,
        config: Config,
    ) -> Self {
        Self {
            config,
            format: format.to_string(),
            project: project.to_string(),
            network: network.to_string(),
            include_raw,
            verbose,
            backend_override: backend,
        }
    }
}
