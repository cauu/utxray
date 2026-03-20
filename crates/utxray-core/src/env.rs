use serde::Serialize;

use crate::config::Config;

#[derive(Debug, Serialize)]
pub struct EnvInfo {
    pub aiken: AikenInfo,
    pub config: ConfigInfo,
    pub backends: BackendsInfo,
}

#[derive(Debug, Serialize)]
pub struct AikenInfo {
    pub installed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ConfigInfo {
    pub loaded: bool,
    pub project: String,
    pub network: String,
}

#[derive(Debug, Serialize)]
pub struct BackendsInfo {
    pub blockfrost: BackendStatus,
    pub ogmios: BackendStatus,
}

#[derive(Debug, Serialize)]
pub struct BackendStatus {
    pub configured: bool,
}

/// Check the environment: aiken installation, config status, backends.
pub async fn check_env(
    config: &Config,
    project_dir: &str,
    config_loaded: bool,
) -> Result<EnvInfo, anyhow::Error> {
    let aiken = check_aiken().await;

    let config_info = ConfigInfo {
        loaded: config_loaded,
        project: project_dir.to_string(),
        network: config.network.default.clone(),
    };

    let backends = BackendsInfo {
        blockfrost: BackendStatus {
            configured: config.blockfrost.project_id.is_some(),
        },
        ogmios: BackendStatus {
            // Ogmios is considered configured if it has non-default or any host/port set
            // For now, we just check if the config section exists (it always does with defaults)
            // A more nuanced check would verify connectivity, but that's not env's job
            configured: false,
        },
    };

    Ok(EnvInfo {
        aiken,
        config: config_info,
        backends,
    })
}

async fn check_aiken() -> AikenInfo {
    // Check if aiken is in PATH
    let aiken_path = which::which("aiken").ok();

    if let Some(path) = aiken_path {
        let path_str = path.to_string_lossy().to_string();

        // Try to get version
        let version = match tokio::process::Command::new("aiken")
            .arg("--version")
            .output()
            .await
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                // aiken --version outputs something like "aiken v1.1.17"
                let version = stdout
                    .trim()
                    .strip_prefix("aiken ")
                    .or_else(|| stdout.trim().strip_prefix("aiken v"))
                    .unwrap_or(stdout.trim())
                    .to_string();
                if version.is_empty() {
                    None
                } else {
                    Some(version)
                }
            }
            Err(_) => None,
        };

        AikenInfo {
            installed: true,
            version,
            path: Some(path_str),
        }
    } else {
        AikenInfo {
            installed: false,
            version: None,
            path: None,
        }
    }
}
