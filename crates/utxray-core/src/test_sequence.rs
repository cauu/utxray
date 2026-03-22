use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::output::Output;

// ── Error types ────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum TestSequenceError {
    #[error("spec file not found: {0}")]
    SpecNotFound(String),

    #[error("invalid spec JSON: {0}")]
    InvalidSpec(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ── Input types (sequence spec) ────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SequenceSpec {
    #[serde(default)]
    pub description: Option<String>,
    pub steps: Vec<StepSpec>,
}

#[derive(Debug, Deserialize)]
pub struct StepSpec {
    #[serde(default = "default_step_number")]
    pub step: usize,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub action: Option<String>,
    /// Expected result: "pass" or "fail"
    #[serde(default)]
    pub expect: Option<String>,
    /// Arbitrary tx data for this step
    #[serde(default)]
    pub tx: Option<serde_json::Value>,
}

fn default_step_number() -> usize {
    0
}

// ── Output types ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TestSequenceOutput {
    pub steps: Vec<StepResult>,
    pub passed: usize,
    pub failed: usize,
    pub total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StepResult {
    pub step: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ── Public API ─────────────────────────────────────────────────

/// Run a test sequence from a spec file.
///
/// For v1, this parses the sequence spec and runs through each step,
/// returning per-step status. Actual tx evaluation is simulated.
pub async fn run_sequence(
    spec_path: &str,
    _project_dir: &str,
) -> anyhow::Result<Output<serde_json::Value>> {
    // 1. Check file exists
    let path = Path::new(spec_path);
    if !path.exists() {
        let output = Output::error(serde_json::json!({
            "error_code": "SPEC_NOT_FOUND",
            "message": format!("Sequence spec file not found: {}", spec_path),
        }));
        return Ok(output);
    }

    // 2. Read and parse
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("failed to read spec file: {e}"))?;

    let spec: SequenceSpec = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(e) => {
            let output = Output::error(serde_json::json!({
                "error_code": "INVALID_SPEC",
                "message": format!("Invalid sequence spec JSON: {}", e),
            }));
            return Ok(output);
        }
    };

    // 3. Run each step
    let mut step_results = Vec::new();
    let mut passed = 0usize;
    let mut failed = 0usize;

    for (idx, step) in spec.steps.iter().enumerate() {
        let step_num = if step.step > 0 { step.step } else { idx + 1 };
        let result = execute_step(step, step_num).await;
        match &result.result {
            Some(r) if r == "pass" => passed += 1,
            Some(r) if r == "fail" => failed += 1,
            _ => {}
        }
        step_results.push(result);
    }

    let total = step_results.len();

    let data = serde_json::json!({
        "steps": step_results.iter().map(|s| serde_json::json!({
            "step": s.step,
            "description": s.description,
            "status": s.status,
            "result": s.result,
            "error": s.error,
        })).collect::<Vec<_>>(),
        "passed": passed,
        "failed": failed,
        "total": total,
        "description": spec.description,
    });

    let output = if failed > 0 {
        Output::mixed(data)
    } else {
        Output::ok(data)
    };

    Ok(output)
}

// ── Internal helpers ───────────────────────────────────────────

async fn execute_step(step: &StepSpec, step_num: usize) -> StepResult {
    // v1: basic step execution
    // In a full implementation, this would:
    // 1. Build the transaction from step.tx
    // 2. Evaluate against current UTxO state
    // 3. Update state with results
    //
    // For now, we validate the step spec structure and return ok
    // unless the step explicitly expects failure

    let description = step.description.clone();

    if step.action.is_none() && step.tx.is_none() {
        return StepResult {
            step: step_num,
            description,
            status: "ok".to_string(),
            result: Some("pass".to_string()),
            error: None,
        };
    }

    // If the step has an expected result, honor it for testing purposes
    let result = match &step.expect {
        Some(e) if e == "fail" => "fail".to_string(),
        _ => "pass".to_string(),
    };

    StepResult {
        step: step_num,
        description,
        status: "ok".to_string(),
        result: Some(result),
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[tokio::test]
    async fn test_run_sequence_file_not_found() -> TestResult {
        let output = run_sequence("/nonexistent/spec.json", ".").await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "SPEC_NOT_FOUND");
        Ok(())
    }

    #[tokio::test]
    async fn test_run_sequence_invalid_json() -> TestResult {
        let mut tmp = tempfile::NamedTempFile::new()?;
        write!(tmp, "not valid json {{{{")?;
        let path = tmp.path().to_str().ok_or("non-utf8 path")?;

        let output = run_sequence(path, ".").await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "INVALID_SPEC");
        Ok(())
    }

    #[tokio::test]
    async fn test_run_sequence_valid_spec() -> TestResult {
        let spec = serde_json::json!({
            "description": "test escrow flow",
            "steps": [
                {"step": 1, "description": "lock escrow", "action": "lock"},
                {"step": 2, "description": "unlock escrow", "action": "unlock"}
            ]
        });

        let mut tmp = tempfile::NamedTempFile::new()?;
        write!(tmp, "{}", serde_json::to_string(&spec)?)?;
        let path = tmp.path().to_str().ok_or("non-utf8 path")?;

        let output = run_sequence(path, ".").await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "ok");
        assert_eq!(json["passed"], 2);
        assert_eq!(json["failed"], 0);
        assert_eq!(json["total"], 2);

        let steps = json["steps"].as_array().ok_or("expected steps array")?;
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0]["step"], 1);
        assert_eq!(steps[0]["description"], "lock escrow");
        assert_eq!(steps[0]["status"], "ok");
        Ok(())
    }

    #[tokio::test]
    async fn test_run_sequence_mixed_results() -> TestResult {
        let spec = serde_json::json!({
            "steps": [
                {"step": 1, "description": "should pass", "action": "lock"},
                {"step": 2, "description": "should fail", "action": "unlock", "expect": "fail"}
            ]
        });

        let mut tmp = tempfile::NamedTempFile::new()?;
        write!(tmp, "{}", serde_json::to_string(&spec)?)?;
        let path = tmp.path().to_str().ok_or("non-utf8 path")?;

        let output = run_sequence(path, ".").await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "mixed");
        assert_eq!(json["passed"], 1);
        assert_eq!(json["failed"], 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_run_sequence_empty_steps() -> TestResult {
        let spec = serde_json::json!({
            "steps": []
        });

        let mut tmp = tempfile::NamedTempFile::new()?;
        write!(tmp, "{}", serde_json::to_string(&spec)?)?;
        let path = tmp.path().to_str().ok_or("non-utf8 path")?;

        let output = run_sequence(path, ".").await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "ok");
        assert_eq!(json["total"], 0);
        Ok(())
    }

    #[test]
    fn test_sequence_spec_deserialize() -> TestResult {
        let json = r#"{"steps": [{"step": 1, "description": "test"}]}"#;
        let spec: SequenceSpec = serde_json::from_str(json)?;
        assert_eq!(spec.steps.len(), 1);
        assert_eq!(spec.steps[0].step, 1);
        Ok(())
    }
}
