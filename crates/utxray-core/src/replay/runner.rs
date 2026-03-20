use serde::Serialize;

use crate::aiken::cli::AikenCli;
use crate::output::Output;

use super::bundle::ReplayBundle;

/// Errors specific to the replay run command.
#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
    #[error("--bundle is required: provide a bundle file path")]
    BundleRequired,

    #[error("failed to read bundle file '{0}': {1}")]
    FileReadError(String, String),

    #[error("bundle is not valid JSON: {0}")]
    InvalidJson(String),

    #[error("bundle structure is invalid: {0}")]
    InvalidBundle(String),
}

/// Version match info for a single component.
#[derive(Debug, Serialize)]
pub struct VersionMatch {
    pub ok: bool,
    pub bundled: String,
    pub current: String,
}

/// Environment match info.
#[derive(Debug, Serialize)]
pub struct EnvironmentMatch {
    pub aiken_version: VersionMatch,
}

/// Output data for the replay run command.
#[derive(Debug, Serialize)]
pub struct RunOutput {
    pub environment_match: EnvironmentMatch,
    pub execution: ExecutionResult,
    pub traces: Vec<String>,
}

/// Result of re-executing the bundled command.
#[derive(Debug, Serialize)]
pub struct ExecutionResult {
    pub command: String,
    pub re_executed: bool,
    pub result: serde_json::Value,
}

/// Detect current aiken version.
async fn detect_current_aiken_version() -> String {
    match AikenCli::new(".") {
        Ok(_) => {
            let output = tokio::process::Command::new("aiken")
                .arg("--version")
                .output()
                .await;
            match output {
                Ok(o) => {
                    let stdout = String::from_utf8_lossy(&o.stdout);
                    let version = stdout.trim().to_string();
                    if version.is_empty() {
                        "unknown".to_string()
                    } else {
                        version
                    }
                }
                Err(_) => "not installed".to_string(),
            }
        }
        Err(_) => "not installed".to_string(),
    }
}

/// Run a replay bundle.
///
/// Parses the bundle, checks environment compatibility, and attempts
/// to re-execute the bundled command.
pub async fn run_bundle(
    bundle_path: Option<&str>,
    _project_dir: &str,
) -> anyhow::Result<Output<serde_json::Value>> {
    let bundle_path = match bundle_path {
        Some(p) => p,
        None => {
            let output = Output::error(serde_json::json!({
                "error_code": "INVALID_INPUT",
                "message": RunnerError::BundleRequired.to_string()
            }));
            return Ok(output);
        }
    };

    // Read and parse the bundle
    let content = match std::fs::read_to_string(bundle_path) {
        Ok(c) => c,
        Err(e) => {
            let output = Output::error(serde_json::json!({
                "error_code": "FILE_READ_ERROR",
                "message": RunnerError::FileReadError(bundle_path.to_string(), e.to_string()).to_string()
            }));
            return Ok(output);
        }
    };

    let bundle: ReplayBundle = match serde_json::from_str(&content) {
        Ok(b) => b,
        Err(e) => {
            // Try parsing as generic JSON first to give better error
            let is_json = serde_json::from_str::<serde_json::Value>(&content).is_ok();
            let (code, msg) = if is_json {
                (
                    "INVALID_BUNDLE",
                    RunnerError::InvalidBundle(e.to_string()).to_string(),
                )
            } else {
                (
                    "INVALID_JSON",
                    RunnerError::InvalidJson(e.to_string()).to_string(),
                )
            };
            let output = Output::error(serde_json::json!({
                "error_code": code,
                "message": msg
            }));
            return Ok(output);
        }
    };

    // Check environment: aiken version
    let current_aiken = detect_current_aiken_version().await;
    let bundled_aiken = bundle.build_artifacts.aiken_version.clone();
    let aiken_match = current_aiken == bundled_aiken;

    let env_match = EnvironmentMatch {
        aiken_version: VersionMatch {
            ok: aiken_match,
            bundled: bundled_aiken,
            current: current_aiken,
        },
    };

    // Extract traces from the bundled result
    let traces: Vec<String> = bundle
        .execution
        .result
        .get("traces")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .or_else(|| {
            bundle
                .execution
                .result
                .get("results")
                .and_then(|v| v.as_array())
                .map(|results| {
                    results
                        .iter()
                        .flat_map(|r| {
                            r.get("traces")
                                .and_then(|t| t.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default()
                        })
                        .collect()
                })
        })
        .unwrap_or_default();

    let run_output = RunOutput {
        environment_match: env_match,
        execution: ExecutionResult {
            command: bundle.execution.command,
            re_executed: false,
            result: bundle.execution.result,
        },
        traces,
    };

    let data = serde_json::to_value(&run_output)
        .map_err(|e| anyhow::anyhow!("failed to serialize run output: {e}"))?;

    Ok(Output::ok(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[tokio::test]
    async fn test_run_bundle_no_path() -> TestResult {
        let output = run_bundle(None, ".").await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "INVALID_INPUT");
        Ok(())
    }

    #[tokio::test]
    async fn test_run_bundle_missing_file() -> TestResult {
        let output = run_bundle(Some("/nonexistent/bundle.json"), ".").await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "FILE_READ_ERROR");
        Ok(())
    }

    #[tokio::test]
    async fn test_run_bundle_invalid_json() -> TestResult {
        let mut tmpfile = tempfile::NamedTempFile::new()?;
        write!(tmpfile, "not json")?;
        let path = tmpfile.path().to_str().ok_or("non-utf8 path")?;
        let output = run_bundle(Some(path), ".").await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "INVALID_JSON");
        Ok(())
    }

    #[tokio::test]
    async fn test_run_bundle_invalid_structure() -> TestResult {
        let mut tmpfile = tempfile::NamedTempFile::new()?;
        write!(tmpfile, r#"{{"key": "value"}}"#)?;
        let path = tmpfile.path().to_str().ok_or("non-utf8 path")?;
        let output = run_bundle(Some(path), ".").await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "INVALID_BUNDLE");
        Ok(())
    }

    #[tokio::test]
    async fn test_run_bundle_success() -> TestResult {
        let bundle = serde_json::json!({
            "v": "0.1.0",
            "created_at": "2025-03-20T10:30:00Z",
            "build_artifacts": {
                "aiken_version": "aiken v1.1.17",
                "trace_level": "verbose",
                "build_mode": "check"
            },
            "chain_snapshot": {
                "network": "preview",
                "era": "Conway",
                "protocol_params": {},
                "utxo_set": []
            },
            "execution": {
                "command": "test",
                "args": {},
                "result": {
                    "total": 1,
                    "results": [
                        {
                            "result": "fail",
                            "traces": ["deadline check failed"]
                        }
                    ]
                }
            }
        });

        let mut tmpfile = tempfile::NamedTempFile::new()?;
        write!(tmpfile, "{}", serde_json::to_string(&bundle)?)?;
        let path = tmpfile.path().to_str().ok_or("non-utf8 path")?;

        let output = run_bundle(Some(path), ".").await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "ok");
        assert!(json["environment_match"]["aiken_version"].is_object());
        assert!(json["environment_match"]["aiken_version"]["bundled"]
            .as_str()
            .is_some());
        assert!(json["environment_match"]["aiken_version"]["current"]
            .as_str()
            .is_some());
        assert_eq!(json["execution"]["command"], "test");
        assert_eq!(json["execution"]["re_executed"], false);
        assert!(json["traces"].is_array());
        let traces = json["traces"].as_array().ok_or("expected array")?;
        assert_eq!(traces.len(), 1);
        assert_eq!(traces[0], "deadline check failed");
        Ok(())
    }
}
