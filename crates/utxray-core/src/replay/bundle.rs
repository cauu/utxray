use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::aiken::cli::AikenCli;
use crate::output::Output;

/// Errors specific to the replay bundle command.
#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("--from is required: provide a result JSON file path")]
    FromRequired,

    #[error("failed to read file '{0}': {1}")]
    FileReadError(String, String),

    #[error("input is not valid JSON: {0}")]
    InvalidJson(String),

    #[error("failed to write bundle to '{0}': {1}")]
    WriteError(String, String),
}

/// The complete replay bundle structure.
#[derive(Debug, Serialize, Deserialize)]
pub struct ReplayBundle {
    pub v: String,
    pub created_at: String,
    pub build_artifacts: BuildArtifacts,
    pub chain_snapshot: ChainSnapshot,
    pub execution: Execution,
}

/// Build artifacts section of the bundle.
#[derive(Debug, Serialize, Deserialize)]
pub struct BuildArtifacts {
    pub aiken_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plutus_json: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aiken_toml: Option<serde_json::Value>,
    pub trace_level: String,
    pub build_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_revision: Option<String>,
}

/// Chain snapshot section of the bundle.
#[derive(Debug, Serialize, Deserialize)]
pub struct ChainSnapshot {
    pub network: String,
    pub era: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<u64>,
    pub protocol_params: serde_json::Value,
    pub utxo_set: Vec<serde_json::Value>,
}

/// Execution section of the bundle.
#[derive(Debug, Serialize, Deserialize)]
pub struct Execution {
    pub command: String,
    pub args: serde_json::Value,
    pub result: serde_json::Value,
}

/// Output data for the replay bundle command.
#[derive(Debug, Serialize)]
pub struct BundleOutput {
    pub bundle_path: String,
    pub build_artifacts: BundleArtifactsSummary,
    pub chain_snapshot: BundleSnapshotSummary,
}

#[derive(Debug, Serialize)]
pub struct BundleArtifactsSummary {
    pub aiken_version: String,
    pub has_plutus_json: bool,
    pub has_aiken_toml: bool,
}

#[derive(Debug, Serialize)]
pub struct BundleSnapshotSummary {
    pub network: String,
    pub has_protocol_params: bool,
    pub utxo_count: usize,
}

/// Detect aiken version by running `aiken --version`.
async fn detect_aiken_version() -> String {
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
                Err(_) => "unknown".to_string(),
            }
        }
        Err(_) => "unknown".to_string(),
    }
}

/// Get git revision if available.
fn detect_git_revision(project_dir: &str) -> Option<String> {
    let git_head = Path::new(project_dir).join(".git/HEAD");
    if git_head.exists() {
        if let Ok(content) = std::fs::read_to_string(&git_head) {
            let trimmed = content.trim();
            if let Some(ref_name) = trimmed.strip_prefix("ref: ") {
                // It's a symbolic ref, try to read the actual hash
                let ref_path = Path::new(project_dir).join(".git").join(ref_name);
                if let Ok(hash) = std::fs::read_to_string(ref_path) {
                    let short = hash.trim().chars().take(7).collect::<String>();
                    return Some(format!("git:{short}"));
                }
            } else {
                let short = trimmed.chars().take(7).collect::<String>();
                return Some(format!("git:{short}"));
            }
        }
    }
    None
}

/// Read and parse plutus.json from the project directory.
fn read_plutus_json(project_dir: &str) -> Option<serde_json::Value> {
    let path = Path::new(project_dir).join("plutus.json");
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            return serde_json::from_str(&content).ok();
        }
    }
    None
}

/// Read aiken.toml and return it as a JSON value.
fn read_aiken_toml(project_dir: &str) -> Option<serde_json::Value> {
    let path = Path::new(project_dir).join("aiken.toml");
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(val) = content.parse::<toml::Value>() {
                // Convert toml::Value to serde_json::Value
                if let Ok(json_str) = serde_json::to_string(&val) {
                    return serde_json::from_str(&json_str).ok();
                }
            }
        }
    }
    None
}

/// Detect the command that produced the result JSON.
fn detect_command(input: &serde_json::Value) -> String {
    if let Some(exec) = input.get("execution") {
        if let Some(cmd) = exec.get("command").and_then(|v| v.as_str()) {
            return cmd.to_string();
        }
    }
    if input.get("results").is_some() && input.get("total").is_some() {
        return "test".to_string();
    }
    if input.get("traces").is_some() && input.get("validator").is_some() {
        return "trace".to_string();
    }
    "unknown".to_string()
}

/// Create a replay bundle from a result JSON file.
pub async fn create_bundle(
    from: Option<&str>,
    _tx: Option<&str>,
    output_path: Option<&str>,
    project_dir: &str,
    network: &str,
) -> anyhow::Result<Output<serde_json::Value>> {
    let from = match from {
        Some(f) => f,
        None => {
            let output = Output::error(serde_json::json!({
                "error_code": "INVALID_INPUT",
                "message": BundleError::FromRequired.to_string()
            }));
            return Ok(output);
        }
    };

    // Read the result JSON
    let content = match std::fs::read_to_string(from) {
        Ok(c) => c,
        Err(e) => {
            let output = Output::error(serde_json::json!({
                "error_code": "FILE_READ_ERROR",
                "message": BundleError::FileReadError(from.to_string(), e.to_string()).to_string()
            }));
            return Ok(output);
        }
    };

    let result_json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            let output = Output::error(serde_json::json!({
                "error_code": "INVALID_JSON",
                "message": BundleError::InvalidJson(e.to_string()).to_string()
            }));
            return Ok(output);
        }
    };

    // Gather build artifacts
    let aiken_version = detect_aiken_version().await;
    let plutus_json = read_plutus_json(project_dir);
    let aiken_toml = read_aiken_toml(project_dir);
    let source_revision = detect_git_revision(project_dir);
    let command = detect_command(&result_json);

    let bundle = ReplayBundle {
        v: crate::output::UTXRAY_VERSION.to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        build_artifacts: BuildArtifacts {
            aiken_version: aiken_version.clone(),
            plutus_json: plutus_json.clone(),
            aiken_toml: aiken_toml.clone(),
            trace_level: "verbose".to_string(),
            build_mode: "check".to_string(),
            script_hash: None,
            source_revision,
        },
        chain_snapshot: ChainSnapshot {
            network: network.to_string(),
            era: "Conway".to_string(),
            slot: None,
            protocol_params: serde_json::json!({}),
            utxo_set: vec![],
        },
        execution: Execution {
            command,
            args: serde_json::json!({}),
            result: result_json,
        },
    };

    // Write bundle to file
    let dest = output_path.unwrap_or("replay.bundle.json");
    let bundle_json = serde_json::to_string_pretty(&bundle)
        .map_err(|e| anyhow::anyhow!("failed to serialize bundle: {e}"))?;

    if let Err(e) = std::fs::write(dest, &bundle_json) {
        let output = Output::error(serde_json::json!({
            "error_code": "WRITE_ERROR",
            "message": BundleError::WriteError(dest.to_string(), e.to_string()).to_string()
        }));
        return Ok(output);
    }

    let bundle_output = BundleOutput {
        bundle_path: dest.to_string(),
        build_artifacts: BundleArtifactsSummary {
            aiken_version,
            has_plutus_json: plutus_json.is_some(),
            has_aiken_toml: aiken_toml.is_some(),
        },
        chain_snapshot: BundleSnapshotSummary {
            network: network.to_string(),
            has_protocol_params: true,
            utxo_count: 0,
        },
    };

    let data = serde_json::to_value(&bundle_output)
        .map_err(|e| anyhow::anyhow!("failed to serialize bundle output: {e}"))?;

    Ok(Output::ok(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[tokio::test]
    async fn test_create_bundle_no_from() -> TestResult {
        let output = create_bundle(None, None, None, ".", "preview").await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "INVALID_INPUT");
        Ok(())
    }

    #[tokio::test]
    async fn test_create_bundle_missing_file() -> TestResult {
        let output =
            create_bundle(Some("/nonexistent/file.json"), None, None, ".", "preview").await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "FILE_READ_ERROR");
        Ok(())
    }

    #[tokio::test]
    async fn test_create_bundle_invalid_json() -> TestResult {
        let mut tmpfile = tempfile::NamedTempFile::new()?;
        write!(tmpfile, "not valid json")?;
        let path = tmpfile.path().to_str().ok_or("non-utf8 path")?;
        let output = create_bundle(Some(path), None, None, ".", "preview").await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "INVALID_JSON");
        Ok(())
    }

    #[tokio::test]
    async fn test_create_bundle_success() -> TestResult {
        let mut input_file = tempfile::NamedTempFile::new()?;
        let input = serde_json::json!({
            "total": 1,
            "results": [{"result": "fail", "traces": ["deadline check failed"]}]
        });
        write!(input_file, "{}", serde_json::to_string(&input)?)?;

        let output_dir = tempfile::TempDir::new()?;
        let output_path = output_dir.path().join("test.bundle.json");
        let output_str = output_path.to_str().ok_or("non-utf8 path")?;

        let input_path = input_file.path().to_str().ok_or("non-utf8 path")?;
        let output =
            create_bundle(Some(input_path), None, Some(output_str), ".", "preview").await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "ok");
        assert_eq!(json["bundle_path"], output_str);
        assert!(json["build_artifacts"]["aiken_version"].is_string());

        // Verify the bundle file was written and is valid JSON
        let bundle_content = std::fs::read_to_string(&output_path)?;
        let bundle: serde_json::Value = serde_json::from_str(&bundle_content)?;
        assert_eq!(bundle["v"], "0.1.0");
        assert!(bundle["created_at"].is_string());
        assert!(bundle["build_artifacts"]["aiken_version"].is_string());
        assert!(bundle["chain_snapshot"]["protocol_params"].is_object());
        assert!(bundle["execution"]["result"].is_object());
        Ok(())
    }

    #[test]
    fn test_detect_command_test() {
        let input = serde_json::json!({ "results": [], "total": 0 });
        assert_eq!(detect_command(&input), "test");
    }

    #[test]
    fn test_detect_command_trace() {
        let input = serde_json::json!({ "traces": [], "validator": "foo" });
        assert_eq!(detect_command(&input), "trace");
    }

    #[test]
    fn test_read_plutus_json_nonexistent() {
        assert!(read_plutus_json("/nonexistent/path").is_none());
    }

    #[test]
    fn test_read_aiken_toml_nonexistent() {
        assert!(read_aiken_toml("/nonexistent/path").is_none());
    }
}
