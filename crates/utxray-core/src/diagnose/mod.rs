pub mod classifier;

use std::io::Read;
use std::path::Path;

use serde::Serialize;

use crate::error::{Confidence, ErrorCode, Severity};
use crate::output::Output;

use classifier::Classification;

/// Errors specific to the diagnose command.
#[derive(Debug, thiserror::Error)]
pub enum DiagnoseError {
    #[error("--from is required: provide a file path or '-' for stdin")]
    FromRequired,

    #[error("failed to read file '{0}': {1}")]
    FileReadError(String, String),

    #[error("failed to read stdin: {0}")]
    StdinReadError(String),

    #[error("input is not valid JSON: {0}")]
    InvalidJson(String),
}

/// Output data for the diagnose command.
#[derive(Debug, Serialize)]
pub struct DiagnoseOutput {
    pub error_code: ErrorCode,
    pub severity: Severity,
    pub category: String,
    pub confidence: Confidence,
    pub source_command: String,
    pub matched_rules: Vec<String>,
    pub summary: String,
    pub evidence: serde_json::Value,
    pub suggested_commands: Vec<String>,
    pub related_errors: Vec<String>,
}

impl From<Classification> for DiagnoseOutput {
    fn from(c: Classification) -> Self {
        Self {
            error_code: c.error_code,
            severity: c.severity,
            category: c.category,
            confidence: c.confidence,
            source_command: c.source_command,
            matched_rules: c.matched_rules,
            summary: c.summary,
            evidence: c.evidence,
            suggested_commands: c.suggested_commands,
            related_errors: c.related_errors,
        }
    }
}

/// Read input JSON from a file path or stdin ("-").
fn read_input(from: &str) -> Result<serde_json::Value, DiagnoseError> {
    let content = if from == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| DiagnoseError::StdinReadError(e.to_string()))?;
        buf
    } else {
        let path = Path::new(from);
        std::fs::read_to_string(path)
            .map_err(|e| DiagnoseError::FileReadError(from.to_string(), e.to_string()))?
    };

    serde_json::from_str(&content).map_err(|e| DiagnoseError::InvalidJson(e.to_string()))
}

/// Run the diagnose command.
///
/// Reads a result JSON file (or stdin), classifies the error, and returns
/// structured diagnostic output.
pub async fn run_diagnose(from: Option<&str>) -> anyhow::Result<Output<serde_json::Value>> {
    let from = match from {
        Some(f) => f,
        None => {
            let output = Output::error(serde_json::json!({
                "error_code": "INVALID_INPUT",
                "message": DiagnoseError::FromRequired.to_string()
            }));
            return Ok(output);
        }
    };

    let input = match read_input(from) {
        Ok(v) => v,
        Err(e) => {
            let (code, msg) = match &e {
                DiagnoseError::FileReadError(_, _) => ("FILE_READ_ERROR", e.to_string()),
                DiagnoseError::StdinReadError(_) => ("STDIN_READ_ERROR", e.to_string()),
                DiagnoseError::InvalidJson(_) => ("INVALID_JSON", e.to_string()),
                DiagnoseError::FromRequired => ("INVALID_INPUT", e.to_string()),
            };
            let output = Output::error(serde_json::json!({
                "error_code": code,
                "message": msg
            }));
            return Ok(output);
        }
    };

    let classification = classifier::classify(&input);
    let diagnose_output = DiagnoseOutput::from(classification);

    let data = serde_json::to_value(&diagnose_output)
        .map_err(|e| anyhow::anyhow!("failed to serialize diagnose output: {e}"))?;

    Ok(Output::ok(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[tokio::test]
    async fn test_diagnose_no_from() -> TestResult {
        let output = run_diagnose(None).await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "INVALID_INPUT");
        Ok(())
    }

    #[tokio::test]
    async fn test_diagnose_missing_file() -> TestResult {
        let output = run_diagnose(Some("/nonexistent/file.json")).await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "FILE_READ_ERROR");
        Ok(())
    }

    #[tokio::test]
    async fn test_diagnose_invalid_json_file() -> TestResult {
        let mut tmpfile = tempfile::NamedTempFile::new()?;
        write!(tmpfile, "not valid json {{")?;
        let path = tmpfile.path().to_str().ok_or("non-utf8 path")?;
        let output = run_diagnose(Some(path)).await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "INVALID_JSON");
        Ok(())
    }

    #[tokio::test]
    async fn test_diagnose_classifies_redeemer_mismatch() -> TestResult {
        let mut tmpfile = tempfile::NamedTempFile::new()?;
        let input = serde_json::json!({
            "message": "Redeemer at index 0 does not match expected input",
            "traces": ["redeemer index mismatch detected"]
        });
        write!(tmpfile, "{}", serde_json::to_string(&input)?)?;
        let path = tmpfile.path().to_str().ok_or("non-utf8 path")?;
        let output = run_diagnose(Some(path)).await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "ok");
        assert_eq!(json["error_code"], "REDEEMER_INDEX_MISMATCH");
        assert_eq!(json["severity"], "critical");
        assert_eq!(json["category"], "cbor_schema");
        assert_eq!(json["confidence"], "high");
        assert!(json["suggested_commands"].is_array());
        assert!(!json["suggested_commands"]
            .as_array()
            .ok_or("expected array")?
            .is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_diagnose_classifies_budget_exceeded() -> TestResult {
        let mut tmpfile = tempfile::NamedTempFile::new()?;
        let input = serde_json::json!({
            "error_detail": "Execution budget exceeded",
            "traces": ["budget limit reached"]
        });
        write!(tmpfile, "{}", serde_json::to_string(&input)?)?;
        let path = tmpfile.path().to_str().ok_or("non-utf8 path")?;
        let output = run_diagnose(Some(path)).await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "ok");
        assert_eq!(json["error_code"], "PHASE2_BUDGET_EXCEEDED");
        Ok(())
    }

    #[tokio::test]
    async fn test_diagnose_classifies_unknown() -> TestResult {
        let mut tmpfile = tempfile::NamedTempFile::new()?;
        let input = serde_json::json!({
            "message": "something completely unrelated happened"
        });
        write!(tmpfile, "{}", serde_json::to_string(&input)?)?;
        let path = tmpfile.path().to_str().ok_or("non-utf8 path")?;
        let output = run_diagnose(Some(path)).await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "ok");
        assert_eq!(json["error_code"], "UNKNOWN_ERROR");
        assert_eq!(json["confidence"], "low");
        Ok(())
    }
}
