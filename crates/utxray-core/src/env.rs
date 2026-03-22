use serde::Serialize;

use crate::backend::blockfrost::BlockfrostBackend;
use crate::config::Config;

#[derive(Debug, Serialize)]
pub struct EnvInfo {
    pub aiken: AikenInfo,
    pub config: ConfigInfo,
    pub backends: BackendsInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blockfrost: Option<BlockfrostInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_tip: Option<NetworkTipInfo>,
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

#[derive(Debug, Serialize)]
pub struct BlockfrostInfo {
    pub available: bool,
    pub network: String,
}

#[derive(Debug, Serialize)]
pub struct NetworkTipInfo {
    pub slot: u64,
    pub epoch: u64,
}

/// Check the environment: aiken installation, config status, backends.
///
/// `network_override` takes precedence over `config.network.default` when provided
/// as a non-empty string. This allows the CLI `--network` flag to override the config.
pub async fn check_env(
    config: &Config,
    project_dir: &str,
    config_loaded: bool,
    network_override: &str,
) -> Result<EnvInfo, anyhow::Error> {
    let aiken = check_aiken().await;

    // Use CLI --network override if provided; otherwise fall back to config default
    let effective_network = if network_override.is_empty() {
        config.network.default.clone()
    } else {
        network_override.to_string()
    };

    let config_info = ConfigInfo {
        loaded: config_loaded,
        project: project_dir.to_string(),
        network: effective_network.clone(),
    };

    let bf_configured = config.blockfrost.project_id.is_some();

    let backends = BackendsInfo {
        blockfrost: BackendStatus {
            configured: bf_configured,
        },
        ogmios: BackendStatus { configured: false },
    };

    // If blockfrost is configured, try a health check and tip query
    let (blockfrost_info, network_tip) = if let Some(ref project_id) = config.blockfrost.project_id
    {
        let network = &effective_network;
        match BlockfrostBackend::new(project_id, network) {
            Ok(backend) => {
                let healthy = backend.health().await.unwrap_or(false);
                let tip = if healthy {
                    backend.query_tip().await.ok().map(|t| NetworkTipInfo {
                        slot: t.slot,
                        epoch: t.epoch,
                    })
                } else {
                    None
                };
                (
                    Some(BlockfrostInfo {
                        available: healthy,
                        network: network.clone(),
                    }),
                    tip,
                )
            }
            Err(_) => (
                Some(BlockfrostInfo {
                    available: false,
                    network: network.clone(),
                }),
                None,
            ),
        }
    } else {
        (None, None)
    };

    Ok(EnvInfo {
        aiken,
        config: config_info,
        backends,
        blockfrost: blockfrost_info,
        network_tip,
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
