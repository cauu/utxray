use std::path::Path;

use serde::Serialize;

use crate::aiken::cli::AikenCli;
use crate::output::Output;

// ── Error types ────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum UplcError {
    #[error("UPLC file not found: {0}")]
    FileNotFound(String),

    #[error("aiken not available: {0}")]
    AikenNotAvailable(String),

    #[error("evaluation failed: {0}")]
    EvalFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ── Output types ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct UplcEvalOutput {
    pub result: String,
    pub exec_units: ExecUnits,
    pub budget_source: String,
    pub traces: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_output: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExecUnits {
    pub cpu: u64,
    pub mem: u64,
}

#[derive(Debug, Serialize)]
pub struct UplcErrorData {
    pub error_code: String,
    pub message: String,
}

// ── Public API ─────────────────────────────────────────────────

/// Evaluate a UPLC program file.
///
/// Delegates to `aiken` CLI if available. Returns structured output
/// with result, execution units, and traces.
pub async fn eval(
    file: &str,
    args: Option<&str>,
    project_dir: &str,
    verbose: bool,
) -> anyhow::Result<Output<serde_json::Value>> {
    // 1. Verify the UPLC file exists
    let file_path = Path::new(file);
    if !file_path.exists() {
        let output = Output::error(serde_json::json!({
            "error_code": "FILE_NOT_FOUND",
            "message": format!("UPLC file not found: {}", file),
        }));
        return Ok(output);
    }

    // 2. Check if aiken is available
    let cli = match AikenCli::new(project_dir) {
        Ok(c) => c,
        Err(e) => {
            let output = Output::error(serde_json::json!({
                "error_code": "AIKEN_NOT_AVAILABLE",
                "message": format!("aiken CLI required for UPLC evaluation: {}", e),
            }));
            return Ok(output);
        }
    };

    // 3. Run evaluation via aiken CLI
    let eval_result = run_aiken_uplc_eval(&cli, file, args, project_dir).await;

    match eval_result {
        Ok(result) => {
            let mut data = serde_json::json!({
                "result": result.result,
                "exec_units": {
                    "cpu": result.exec_units.cpu,
                    "mem": result.exec_units.mem,
                },
                "budget_source": result.budget_source,
                "traces": result.traces,
            });

            if verbose {
                if let Some(raw) = &result.raw_output {
                    data.as_object_mut()
                        .map(|obj| obj.insert("raw_output".to_string(), serde_json::json!(raw)));
                }
            }

            let output = if result.result == "pass" {
                Output::ok(data)
            } else {
                Output::mixed(data)
            };
            Ok(output)
        }
        Err(e) => {
            let output = Output::error(serde_json::json!({
                "error_code": "EVAL_FAILED",
                "message": e.to_string(),
            }));
            Ok(output)
        }
    }
}

// ── Internal helpers ───────────────────────────────────────────

async fn run_aiken_uplc_eval(
    _cli: &AikenCli,
    file: &str,
    args: Option<&str>,
    project_dir: &str,
) -> Result<UplcEvalOutput, UplcError> {
    // Build the aiken uplc eval command
    let mut cmd = tokio::process::Command::new("aiken");
    cmd.arg("uplc");
    cmd.arg("eval");
    cmd.arg(file);

    if let Some(a) = args {
        cmd.arg("--args");
        cmd.arg(a);
    }

    cmd.current_dir(project_dir);

    let output = cmd.output().await.map_err(|e| {
        UplcError::AikenNotAvailable(format!("failed to execute aiken uplc eval: {e}"))
    })?;

    let exit_code = output.status.code().unwrap_or(-1);
    let raw_stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let raw_stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{}\n{}", raw_stdout, raw_stderr);

    if exit_code != 0 {
        // Try to extract useful information from stderr
        let traces = extract_traces(&combined);
        return Ok(UplcEvalOutput {
            result: "fail".to_string(),
            exec_units: ExecUnits { cpu: 0, mem: 0 },
            budget_source: "uplc_eval".to_string(),
            traces,
            raw_output: Some(combined),
        });
    }

    // Parse successful output for exec units and traces
    let exec_units = parse_exec_units(&combined);
    let traces = extract_traces(&combined);

    Ok(UplcEvalOutput {
        result: "pass".to_string(),
        exec_units,
        budget_source: "uplc_eval".to_string(),
        traces,
        raw_output: Some(combined),
    })
}

/// Parse execution units from aiken uplc eval output.
/// Looks for patterns like "cpu: 12345" and "mem: 6789".
fn parse_exec_units(output: &str) -> ExecUnits {
    let mut cpu: u64 = 0;
    let mut mem: u64 = 0;

    for line in output.lines() {
        let trimmed = line.trim().to_lowercase();
        if trimmed.contains("cpu") {
            if let Some(val) = extract_number_after(&trimmed, "cpu") {
                cpu = val;
            }
        }
        if trimmed.contains("mem") {
            if let Some(val) = extract_number_after(&trimmed, "mem") {
                mem = val;
            }
        }
    }

    ExecUnits { cpu, mem }
}

fn extract_number_after(line: &str, key: &str) -> Option<u64> {
    let idx = line.find(key)?;
    let rest = &line[idx + key.len()..];
    // Skip non-digit characters (like ": " or "=")
    let num_start = rest.find(|c: char| c.is_ascii_digit())?;
    let num_str: String = rest[num_start..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    num_str.parse().ok()
}

/// Extract trace lines from output.
fn extract_traces(output: &str) -> Vec<String> {
    let mut traces = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        // Look for trace markers
        if trimmed.starts_with("Trace:") || trimmed.starts_with("trace:") || trimmed.contains("↳")
        {
            let msg = trimmed
                .trim_start_matches("Trace:")
                .trim_start_matches("trace:")
                .replace('↳', "")
                .trim()
                .to_string();
            if !msg.is_empty() {
                traces.push(msg);
            }
        }
    }
    traces
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_parse_exec_units_from_output() {
        let output = "Result: ()\ncpu: 12345\nmem: 6789\n";
        let units = parse_exec_units(output);
        assert_eq!(units.cpu, 12345);
        assert_eq!(units.mem, 6789);
    }

    #[test]
    fn test_parse_exec_units_empty() {
        let units = parse_exec_units("no numbers here");
        assert_eq!(units.cpu, 0);
        assert_eq!(units.mem, 0);
    }

    #[test]
    fn test_extract_traces() {
        let output = "Trace: hello world\nsome other line\ntrace: second trace\n";
        let traces = extract_traces(output);
        assert_eq!(traces.len(), 2);
        assert_eq!(traces[0], "hello world");
        assert_eq!(traces[1], "second trace");
    }

    #[test]
    fn test_extract_traces_empty() {
        let traces = extract_traces("no trace lines here");
        assert!(traces.is_empty());
    }

    #[test]
    fn test_extract_number_after() {
        assert_eq!(extract_number_after("cpu: 12345", "cpu"), Some(12345));
        assert_eq!(extract_number_after("mem=999", "mem"), Some(999));
        assert_eq!(extract_number_after("no numbers", "cpu"), None);
    }

    #[tokio::test]
    async fn test_eval_file_not_found() -> TestResult {
        let output = eval("/nonexistent/file.uplc", None, ".", false).await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "FILE_NOT_FOUND");
        Ok(())
    }
}
