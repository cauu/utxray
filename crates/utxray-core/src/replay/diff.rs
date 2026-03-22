use serde::Serialize;

use crate::output::Output;

/// Errors specific to the replay diff command.
#[derive(Debug, thiserror::Error)]
pub enum DiffError {
    #[error("--before is required: provide a result JSON file path")]
    BeforeRequired,

    #[error("--after is required: provide a result JSON file path")]
    AfterRequired,

    #[error("failed to read file '{0}': {1}")]
    FileReadError(String, String),

    #[error("file '{0}' is not valid JSON: {1}")]
    InvalidJson(String, String),
}

/// The result change between two runs.
#[derive(Debug, Serialize)]
pub struct ResultChange {
    pub before: String,
    pub after: String,
}

/// Delta for a single execution unit dimension (cpu or mem).
#[derive(Debug, Serialize)]
pub struct ExecUnitDelta {
    pub before: i64,
    pub after: i64,
    pub delta: i64,
    pub pct: f64,
}

/// Delta for execution units (cpu + mem).
#[derive(Debug, Serialize)]
pub struct ExecUnitsDelta {
    pub cpu: ExecUnitDelta,
    pub mem: ExecUnitDelta,
}

/// A single line in the trace diff.
#[derive(Debug, Serialize)]
pub struct TraceDiffEntry {
    pub line: usize,
    pub before: Option<String>,
    pub after: Option<String>,
    #[serde(rename = "type")]
    pub diff_type: String,
}

/// Output data for the replay diff command.
#[derive(Debug, Serialize)]
pub struct DiffOutput {
    pub result_change: ResultChange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec_units_delta: Option<ExecUnitsDelta>,
    pub trace_diff: Vec<TraceDiffEntry>,
}

/// Extract the "result" field from a result JSON.
/// Looks for top-level "result", or in "results[0].result", or "outcome".
fn extract_result(value: &serde_json::Value) -> String {
    // Direct "result" field
    if let Some(r) = value.get("result").and_then(|v| v.as_str()) {
        return r.to_string();
    }
    // Check in results array
    if let Some(results) = value.get("results").and_then(|v| v.as_array()) {
        if let Some(first) = results.first() {
            if let Some(r) = first.get("result").and_then(|v| v.as_str()) {
                return r.to_string();
            }
        }
    }
    // Check "outcome" field
    if let Some(o) = value.get("outcome").and_then(|v| v.as_str()) {
        return o.to_string();
    }
    // Check "status" as fallback
    if let Some(s) = value.get("status").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    "unknown".to_string()
}

/// Extract exec_units from a result JSON.
/// Looks for "exec_units" at top level, or in "results[0].exec_units".
fn extract_exec_units(value: &serde_json::Value) -> Option<(i64, i64)> {
    let try_parse = |obj: &serde_json::Value| -> Option<(i64, i64)> {
        let cpu = obj.get("cpu").and_then(|v| v.as_i64())?;
        let mem = obj.get("mem").and_then(|v| v.as_i64())?;
        Some((cpu, mem))
    };

    if let Some(eu) = value.get("exec_units") {
        if let Some(pair) = try_parse(eu) {
            return Some(pair);
        }
    }

    if let Some(results) = value.get("results").and_then(|v| v.as_array()) {
        if let Some(first) = results.first() {
            if let Some(eu) = first.get("exec_units") {
                if let Some(pair) = try_parse(eu) {
                    return Some(pair);
                }
            }
        }
    }

    None
}

/// Extract traces from a result JSON.
/// Looks for "traces" array at top level, or in "results[0].traces".
fn extract_traces(value: &serde_json::Value) -> Vec<String> {
    let try_parse = |arr: &[serde_json::Value]| -> Vec<String> {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    };

    if let Some(traces) = value.get("traces").and_then(|v| v.as_array()) {
        return try_parse(traces);
    }

    if let Some(results) = value.get("results").and_then(|v| v.as_array()) {
        if let Some(first) = results.first() {
            if let Some(traces) = first.get("traces").and_then(|v| v.as_array()) {
                return try_parse(traces);
            }
        }
    }

    vec![]
}

/// Compute a percentage change. Returns 0.0 if before is 0.
fn pct_change(before: i64, after: i64) -> f64 {
    if before == 0 {
        return 0.0;
    }
    let delta = after - before;
    // Round to 1 decimal place
    ((delta as f64 / before as f64) * 1000.0).round() / 10.0
}

/// Compute a line-by-line trace diff between two trace arrays.
fn compute_trace_diff(before: &[String], after: &[String]) -> Vec<TraceDiffEntry> {
    let max_len = before.len().max(after.len());
    let mut entries = Vec::new();

    for i in 0..max_len {
        let b = before.get(i);
        let a = after.get(i);

        let diff_type = match (b, a) {
            (Some(bv), Some(av)) => {
                if bv == av {
                    "unchanged"
                } else {
                    "changed"
                }
            }
            (Some(_), None) => "removed",
            (None, Some(_)) => "added",
            (None, None) => continue,
        };

        // Skip unchanged entries to keep diff concise
        if diff_type == "unchanged" {
            continue;
        }

        entries.push(TraceDiffEntry {
            line: i + 1,
            before: b.cloned(),
            after: a.cloned(),
            diff_type: diff_type.to_string(),
        });
    }

    entries
}

/// Diff two result JSON files and return a structured comparison.
pub fn diff_results(
    before_path: Option<&str>,
    after_path: Option<&str>,
) -> anyhow::Result<Output<serde_json::Value>> {
    let before_path = match before_path {
        Some(p) => p,
        None => {
            return Ok(Output::error(serde_json::json!({
                "error_code": "INVALID_INPUT",
                "message": DiffError::BeforeRequired.to_string()
            })));
        }
    };

    let after_path = match after_path {
        Some(p) => p,
        None => {
            return Ok(Output::error(serde_json::json!({
                "error_code": "INVALID_INPUT",
                "message": DiffError::AfterRequired.to_string()
            })));
        }
    };

    // Read before file
    let before_content = match std::fs::read_to_string(before_path) {
        Ok(c) => c,
        Err(e) => {
            return Ok(Output::error(serde_json::json!({
                "error_code": "FILE_READ_ERROR",
                "message": DiffError::FileReadError(before_path.to_string(), e.to_string()).to_string()
            })));
        }
    };

    // Read after file
    let after_content = match std::fs::read_to_string(after_path) {
        Ok(c) => c,
        Err(e) => {
            return Ok(Output::error(serde_json::json!({
                "error_code": "FILE_READ_ERROR",
                "message": DiffError::FileReadError(after_path.to_string(), e.to_string()).to_string()
            })));
        }
    };

    // Parse JSON
    let before_json: serde_json::Value = match serde_json::from_str(&before_content) {
        Ok(v) => v,
        Err(e) => {
            return Ok(Output::error(serde_json::json!({
                "error_code": "INVALID_JSON",
                "message": DiffError::InvalidJson(before_path.to_string(), e.to_string()).to_string()
            })));
        }
    };

    let after_json: serde_json::Value = match serde_json::from_str(&after_content) {
        Ok(v) => v,
        Err(e) => {
            return Ok(Output::error(serde_json::json!({
                "error_code": "INVALID_JSON",
                "message": DiffError::InvalidJson(after_path.to_string(), e.to_string()).to_string()
            })));
        }
    };

    // Extract fields
    let before_result = extract_result(&before_json);
    let after_result = extract_result(&after_json);

    let before_eu = extract_exec_units(&before_json);
    let after_eu = extract_exec_units(&after_json);

    let before_traces = extract_traces(&before_json);
    let after_traces = extract_traces(&after_json);

    // Build exec_units_delta if both have exec_units
    let exec_units_delta = match (before_eu, after_eu) {
        (Some((b_cpu, b_mem)), Some((a_cpu, a_mem))) => Some(ExecUnitsDelta {
            cpu: ExecUnitDelta {
                before: b_cpu,
                after: a_cpu,
                delta: a_cpu - b_cpu,
                pct: pct_change(b_cpu, a_cpu),
            },
            mem: ExecUnitDelta {
                before: b_mem,
                after: a_mem,
                delta: a_mem - b_mem,
                pct: pct_change(b_mem, a_mem),
            },
        }),
        _ => None,
    };

    let trace_diff = compute_trace_diff(&before_traces, &after_traces);

    let diff_output = DiffOutput {
        result_change: ResultChange {
            before: before_result,
            after: after_result,
        },
        exec_units_delta,
        trace_diff,
    };

    let data = serde_json::to_value(&diff_output)
        .map_err(|e| anyhow::anyhow!("failed to serialize diff output: {e}"))?;

    Ok(Output::ok(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_diff_missing_before() -> TestResult {
        let output = diff_results(None, Some("/tmp/after.json"))?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "INVALID_INPUT");
        Ok(())
    }

    #[test]
    fn test_diff_missing_after() -> TestResult {
        let output = diff_results(Some("/tmp/before.json"), None)?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "INVALID_INPUT");
        Ok(())
    }

    #[test]
    fn test_diff_file_not_found() -> TestResult {
        let output = diff_results(
            Some("/nonexistent/before.json"),
            Some("/nonexistent/after.json"),
        )?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "FILE_READ_ERROR");
        Ok(())
    }

    #[test]
    fn test_diff_invalid_json() -> TestResult {
        let mut tmpfile = tempfile::NamedTempFile::new()?;
        write!(tmpfile, "not json")?;
        let before_path = tmpfile.path().to_str().ok_or("non-utf8 path")?.to_string();

        let mut after_file = tempfile::NamedTempFile::new()?;
        write!(after_file, r#"{{"result": "pass"}}"#)?;
        let after_path = after_file
            .path()
            .to_str()
            .ok_or("non-utf8 path")?
            .to_string();

        let output = diff_results(Some(&before_path), Some(&after_path))?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "INVALID_JSON");
        Ok(())
    }

    #[test]
    fn test_diff_different_outcomes() -> TestResult {
        let mut before_file = tempfile::NamedTempFile::new()?;
        write!(
            before_file,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "result": "fail",
                "exec_units": {"cpu": 1050000, "mem": 30200},
                "traces": ["FAIL: deadline exceeded"]
            }))?
        )?;

        let mut after_file = tempfile::NamedTempFile::new()?;
        write!(
            after_file,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "result": "pass",
                "exec_units": {"cpu": 980000, "mem": 28500},
                "traces": ["deadline check: PASS", "unlock successful"]
            }))?
        )?;

        let before_path = before_file
            .path()
            .to_str()
            .ok_or("non-utf8 path")?
            .to_string();
        let after_path = after_file
            .path()
            .to_str()
            .ok_or("non-utf8 path")?
            .to_string();

        let output = diff_results(Some(&before_path), Some(&after_path))?;
        let json = serde_json::to_value(&output)?;

        assert_eq!(json["status"], "ok");
        assert_eq!(json["result_change"]["before"], "fail");
        assert_eq!(json["result_change"]["after"], "pass");

        // Check exec_units_delta
        let eu = &json["exec_units_delta"];
        assert_eq!(eu["cpu"]["before"], 1050000);
        assert_eq!(eu["cpu"]["after"], 980000);
        assert_eq!(eu["cpu"]["delta"], -70000);
        assert_eq!(eu["mem"]["before"], 30200);
        assert_eq!(eu["mem"]["after"], 28500);
        assert_eq!(eu["mem"]["delta"], -1700);

        // Check trace_diff
        let trace_diff = json["trace_diff"].as_array().ok_or("expected array")?;
        assert!(!trace_diff.is_empty());
        // Line 1 should be "changed" (before had 1 trace, after has different)
        assert_eq!(trace_diff[0]["line"], 1);
        assert_eq!(trace_diff[0]["type"], "changed");

        Ok(())
    }

    #[test]
    fn test_diff_same_result() -> TestResult {
        let mut before_file = tempfile::NamedTempFile::new()?;
        write!(
            before_file,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "result": "pass",
                "traces": ["check ok"]
            }))?
        )?;

        let mut after_file = tempfile::NamedTempFile::new()?;
        write!(
            after_file,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "result": "pass",
                "traces": ["check ok"]
            }))?
        )?;

        let before_path = before_file
            .path()
            .to_str()
            .ok_or("non-utf8 path")?
            .to_string();
        let after_path = after_file
            .path()
            .to_str()
            .ok_or("non-utf8 path")?
            .to_string();

        let output = diff_results(Some(&before_path), Some(&after_path))?;
        let json = serde_json::to_value(&output)?;

        assert_eq!(json["status"], "ok");
        assert_eq!(json["result_change"]["before"], "pass");
        assert_eq!(json["result_change"]["after"], "pass");
        // No exec_units_delta since neither has exec_units
        assert!(json.get("exec_units_delta").is_none());
        // Traces are identical, so trace_diff should be empty
        let trace_diff = json["trace_diff"].as_array().ok_or("expected array")?;
        assert!(trace_diff.is_empty());

        Ok(())
    }

    #[test]
    fn test_diff_no_exec_units() -> TestResult {
        let mut before_file = tempfile::NamedTempFile::new()?;
        write!(
            before_file,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "result": "fail",
                "traces": ["error line"]
            }))?
        )?;

        let mut after_file = tempfile::NamedTempFile::new()?;
        write!(
            after_file,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "result": "pass",
                "traces": ["success line"]
            }))?
        )?;

        let before_path = before_file
            .path()
            .to_str()
            .ok_or("non-utf8 path")?
            .to_string();
        let after_path = after_file
            .path()
            .to_str()
            .ok_or("non-utf8 path")?
            .to_string();

        let output = diff_results(Some(&before_path), Some(&after_path))?;
        let json = serde_json::to_value(&output)?;

        assert_eq!(json["status"], "ok");
        assert!(json.get("exec_units_delta").is_none());

        Ok(())
    }

    #[test]
    fn test_extract_result_from_results_array() {
        let val = serde_json::json!({
            "results": [{"result": "fail", "traces": []}],
            "total": 1
        });
        assert_eq!(extract_result(&val), "fail");
    }

    #[test]
    fn test_extract_exec_units_from_results_array() {
        let val = serde_json::json!({
            "results": [{"result": "pass", "exec_units": {"cpu": 100, "mem": 200}}],
            "total": 1
        });
        assert_eq!(extract_exec_units(&val), Some((100, 200)));
    }

    #[test]
    fn test_pct_change_zero_before() {
        assert_eq!(pct_change(0, 100), 0.0);
    }

    #[test]
    fn test_pct_change_normal() {
        let pct = pct_change(1000, 900);
        assert!((pct - (-10.0)).abs() < 0.01);
    }

    #[test]
    fn test_compute_trace_diff_added_lines() {
        let before = vec!["line1".to_string()];
        let after = vec!["line1".to_string(), "line2".to_string()];
        let diff = compute_trace_diff(&before, &after);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].line, 2);
        assert_eq!(diff[0].diff_type, "added");
        assert!(diff[0].before.is_none());
        assert_eq!(diff[0].after.as_deref(), Some("line2"));
    }

    #[test]
    fn test_compute_trace_diff_removed_lines() {
        let before = vec!["line1".to_string(), "line2".to_string()];
        let after = vec!["line1".to_string()];
        let diff = compute_trace_diff(&before, &after);
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].line, 2);
        assert_eq!(diff[0].diff_type, "removed");
    }
}
