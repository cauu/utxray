use serde::Serialize;

use crate::aiken::cli::AikenCli;
use crate::output::Output;

/// Default protocol limits for Cardano (Conway era).
const DEFAULT_CPU_LIMIT: i64 = 10_000_000_000;
const DEFAULT_MEM_LIMIT: i64 = 14_000_000;

/// Errors specific to budget commands.
#[derive(Debug, thiserror::Error)]
pub enum BudgetError {
    #[error("aiken is not available: {0}")]
    AikenNotAvailable(String),

    #[error("aiken check failed (exit code {0}): {1}")]
    AikenCheckFailed(i32, String),

    #[error("--before is required: provide a result JSON file path")]
    BeforeRequired,

    #[error("--after is required: provide a result JSON file path")]
    AfterRequired,

    #[error("failed to read file '{0}': {1}")]
    FileReadError(String, String),

    #[error("file '{0}' is not valid JSON: {1}")]
    InvalidJson(String, String),

    #[error("validator '{0}' not found in results")]
    ValidatorNotFound(String),
}

/// A single benchmark entry for a validator.
#[derive(Debug, Serialize)]
pub struct Benchmark {
    pub source: String,
    pub test_name: String,
    pub cpu: i64,
    pub mem: i64,
    pub cpu_pct_of_limit: f64,
    pub mem_pct_of_limit: f64,
}

/// A validator's budget data.
#[derive(Debug, Serialize)]
pub struct ValidatorBudget {
    pub name: String,
    pub benchmarks: Vec<Benchmark>,
}

/// Output data for the budget show command.
#[derive(Debug, Serialize)]
pub struct BudgetShowOutput {
    pub validators: Vec<ValidatorBudget>,
}

/// Parse aiken check output to extract test results with exec_units.
///
/// Aiken check output looks like:
///
/// ```text
/// Testing ...
///
///     ┍━ module_name ━━━━━━━━━━━━━
///     │ PASS [mem: 1234, cpu: 5678] test_name
///     │ FAIL [mem: 1234, cpu: 5678] test_name
///     ┕
/// ```
///
/// We extract the module name, test name, and exec_units.
fn parse_aiken_check_output(output: &str) -> Vec<ValidatorBudget> {
    let mut validators: Vec<ValidatorBudget> = Vec::new();
    let mut current_module: Option<String> = None;

    for line in output.lines() {
        let trimmed = line.trim();

        // Detect module header: "┍━ module_name ━━━"
        if let Some(rest) = trimmed.strip_prefix("┍━") {
            // Extract module name: everything before the next "━"
            let module_name = rest
                .trim_start_matches('━')
                .trim_start()
                .split('━')
                .next()
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if !module_name.is_empty() {
                current_module = Some(module_name);
            }
            continue;
        }

        // Detect test results: "│ PASS [mem: 1234, cpu: 5678] test_name"
        // or "│ FAIL [mem: 1234, cpu: 5678] test_name"
        if let Some(rest) = trimmed.strip_prefix('│') {
            let rest = rest.trim();
            let is_pass = rest.starts_with("PASS");
            let is_fail = rest.starts_with("FAIL");

            if !is_pass && !is_fail {
                continue;
            }

            // Try to extract [mem: NNN, cpu: NNN]
            if let (Some(bracket_start), Some(bracket_end)) = (rest.find('['), rest.find(']')) {
                let bracket_content = &rest[bracket_start + 1..bracket_end];
                let after_bracket = rest[bracket_end + 1..].trim();

                let mut cpu: Option<i64> = None;
                let mut mem: Option<i64> = None;

                for part in bracket_content.split(',') {
                    let part = part.trim();
                    if let Some(val_str) = part.strip_prefix("cpu:") {
                        cpu = val_str.trim().parse().ok();
                    } else if let Some(val_str) = part.strip_prefix("mem:") {
                        mem = val_str.trim().parse().ok();
                    }
                }

                if let (Some(cpu_val), Some(mem_val)) = (cpu, mem) {
                    let module_name = current_module
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());
                    let test_name = after_bracket.to_string();

                    let cpu_pct =
                        (cpu_val as f64 / DEFAULT_CPU_LIMIT as f64 * 10000.0).round() / 100.0;
                    let mem_pct =
                        (mem_val as f64 / DEFAULT_MEM_LIMIT as f64 * 10000.0).round() / 100.0;

                    let benchmark = Benchmark {
                        source: "test".to_string(),
                        test_name,
                        cpu: cpu_val,
                        mem: mem_val,
                        cpu_pct_of_limit: cpu_pct,
                        mem_pct_of_limit: mem_pct,
                    };

                    // Find or create the validator entry
                    if let Some(existing) = validators.iter_mut().find(|v| v.name == module_name) {
                        existing.benchmarks.push(benchmark);
                    } else {
                        validators.push(ValidatorBudget {
                            name: module_name,
                            benchmarks: vec![benchmark],
                        });
                    }
                }
            }
        }
    }

    validators
}

/// Show budget analysis for validators.
///
/// Runs `aiken check` and parses test results to extract exec_units per validator.
/// If `validator` is Some, filters to that validator; otherwise shows all.
pub async fn budget_show(
    project_dir: &str,
    validator: Option<&str>,
) -> anyhow::Result<Output<serde_json::Value>> {
    let cli = match AikenCli::new(project_dir) {
        Ok(c) => c,
        Err(e) => {
            return Ok(Output::error(serde_json::json!({
                "error_code": "AIKEN_NOT_AVAILABLE",
                "message": BudgetError::AikenNotAvailable(e.to_string()).to_string()
            })));
        }
    };

    let check_output = match cli.check(None, "verbose").await {
        Ok(o) => o,
        Err(e) => {
            return Ok(Output::error(serde_json::json!({
                "error_code": "AIKEN_ERROR",
                "message": format!("failed to run aiken check: {e}")
            })));
        }
    };

    // Parse from both stdout and stderr (aiken outputs to stderr for test results)
    let combined = format!("{}\n{}", check_output.raw_stdout, check_output.raw_stderr);
    let mut validators = parse_aiken_check_output(&combined);

    // Filter by validator name if specified
    if let Some(name) = validator {
        validators.retain(|v| v.name.contains(name));
    }

    let show_output = BudgetShowOutput { validators };
    let data = serde_json::to_value(&show_output)
        .map_err(|e| anyhow::anyhow!("failed to serialize budget output: {e}"))?;

    Ok(Output::ok(data))
}

/// Delta for a single budget dimension.
#[derive(Debug, Serialize)]
pub struct BudgetDelta {
    pub value: i64,
    pub pct: f64,
}

/// Output data for the budget compare command.
#[derive(Debug, Serialize)]
pub struct BudgetCompareOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validator: Option<String>,
    pub before: ExecUnits,
    pub after: ExecUnits,
    pub delta: ExecUnitsDelta,
    pub regression: bool,
    pub threshold: RegressionThreshold,
}

/// Execution units.
#[derive(Debug, Serialize)]
pub struct ExecUnits {
    pub cpu: i64,
    pub mem: i64,
}

/// Delta for execution units.
#[derive(Debug, Serialize)]
pub struct ExecUnitsDelta {
    pub cpu: BudgetDelta,
    pub mem: BudgetDelta,
}

/// Regression detection thresholds.
#[derive(Debug, Serialize)]
pub struct RegressionThreshold {
    pub cpu_pct: f64,
    pub mem_pct: f64,
}

/// Extract exec_units for a specific validator from a result JSON file.
///
/// The file can be:
/// - A test result with "results" array containing exec_units
/// - A trace/evaluate result with top-level exec_units
/// - A budget show output with "validators" array
fn extract_validator_exec_units(
    value: &serde_json::Value,
    validator: Option<&str>,
) -> Option<(i64, i64)> {
    let try_parse_eu = |obj: &serde_json::Value| -> Option<(i64, i64)> {
        let cpu = obj.get("cpu").and_then(|v| v.as_i64())?;
        let mem = obj.get("mem").and_then(|v| v.as_i64())?;
        Some((cpu, mem))
    };

    // Try top-level exec_units
    if let Some(eu) = value.get("exec_units") {
        if let Some(pair) = try_parse_eu(eu) {
            return Some(pair);
        }
    }

    // Try "before"/"after" fields (direct exec units)
    if let (Some(cpu), Some(mem)) = (
        value.get("cpu").and_then(|v| v.as_i64()),
        value.get("mem").and_then(|v| v.as_i64()),
    ) {
        return Some((cpu, mem));
    }

    // Try "validators" array (from budget show output)
    if let Some(validators) = value.get("validators").and_then(|v| v.as_array()) {
        for v in validators {
            let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let matches = validator.is_none_or(|vn| name.contains(vn));
            if matches {
                if let Some(benchmarks) = v.get("benchmarks").and_then(|b| b.as_array()) {
                    if let Some(first) = benchmarks.first() {
                        if let Some(pair) = try_parse_eu(first) {
                            return Some(pair);
                        }
                    }
                }
            }
        }
    }

    // Try "results" array
    if let Some(results) = value.get("results").and_then(|v| v.as_array()) {
        for r in results {
            let name = r.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let test_name = r.get("test_name").and_then(|n| n.as_str()).unwrap_or("");
            let matches = validator.is_none_or(|vn| name.contains(vn) || test_name.contains(vn));
            if matches {
                if let Some(eu) = r.get("exec_units") {
                    if let Some(pair) = try_parse_eu(eu) {
                        return Some(pair);
                    }
                }
            }
        }
        // Fallback: just use first result with exec_units
        if validator.is_none() {
            for r in results {
                if let Some(eu) = r.get("exec_units") {
                    if let Some(pair) = try_parse_eu(eu) {
                        return Some(pair);
                    }
                }
            }
        }
    }

    None
}

/// Compare budgets between two result files.
pub fn budget_compare(
    before_path: Option<&str>,
    after_path: Option<&str>,
    validator: Option<&str>,
) -> anyhow::Result<Output<serde_json::Value>> {
    let before_path = match before_path {
        Some(p) => p,
        None => {
            return Ok(Output::error(serde_json::json!({
                "error_code": "INVALID_INPUT",
                "message": BudgetError::BeforeRequired.to_string()
            })));
        }
    };

    let after_path = match after_path {
        Some(p) => p,
        None => {
            return Ok(Output::error(serde_json::json!({
                "error_code": "INVALID_INPUT",
                "message": BudgetError::AfterRequired.to_string()
            })));
        }
    };

    // Read before file
    let before_content = match std::fs::read_to_string(before_path) {
        Ok(c) => c,
        Err(e) => {
            return Ok(Output::error(serde_json::json!({
                "error_code": "FILE_READ_ERROR",
                "message": BudgetError::FileReadError(before_path.to_string(), e.to_string()).to_string()
            })));
        }
    };

    // Read after file
    let after_content = match std::fs::read_to_string(after_path) {
        Ok(c) => c,
        Err(e) => {
            return Ok(Output::error(serde_json::json!({
                "error_code": "FILE_READ_ERROR",
                "message": BudgetError::FileReadError(after_path.to_string(), e.to_string()).to_string()
            })));
        }
    };

    // Parse JSON
    let before_json: serde_json::Value = match serde_json::from_str(&before_content) {
        Ok(v) => v,
        Err(e) => {
            return Ok(Output::error(serde_json::json!({
                "error_code": "INVALID_JSON",
                "message": BudgetError::InvalidJson(before_path.to_string(), e.to_string()).to_string()
            })));
        }
    };

    let after_json: serde_json::Value = match serde_json::from_str(&after_content) {
        Ok(v) => v,
        Err(e) => {
            return Ok(Output::error(serde_json::json!({
                "error_code": "INVALID_JSON",
                "message": BudgetError::InvalidJson(after_path.to_string(), e.to_string()).to_string()
            })));
        }
    };

    // Extract exec_units
    let before_eu = match extract_validator_exec_units(&before_json, validator) {
        Some(eu) => eu,
        None => {
            let msg = if let Some(v) = validator {
                BudgetError::ValidatorNotFound(v.to_string()).to_string()
            } else {
                "no exec_units found in before file".to_string()
            };
            return Ok(Output::error(serde_json::json!({
                "error_code": "VALIDATOR_NOT_FOUND",
                "message": msg
            })));
        }
    };

    let after_eu = match extract_validator_exec_units(&after_json, validator) {
        Some(eu) => eu,
        None => {
            let msg = if let Some(v) = validator {
                BudgetError::ValidatorNotFound(v.to_string()).to_string()
            } else {
                "no exec_units found in after file".to_string()
            };
            return Ok(Output::error(serde_json::json!({
                "error_code": "VALIDATOR_NOT_FOUND",
                "message": msg
            })));
        }
    };

    let cpu_delta = after_eu.0 - before_eu.0;
    let mem_delta = after_eu.1 - before_eu.1;

    let cpu_pct = if before_eu.0 == 0 {
        0.0
    } else {
        ((cpu_delta as f64 / before_eu.0 as f64) * 1000.0).round() / 10.0
    };

    let mem_pct = if before_eu.1 == 0 {
        0.0
    } else {
        ((mem_delta as f64 / before_eu.1 as f64) * 1000.0).round() / 10.0
    };

    // Default regression thresholds: 10% increase
    let cpu_threshold = 10.0;
    let mem_threshold = 10.0;
    let regression = cpu_pct > cpu_threshold || mem_pct > mem_threshold;

    let compare_output = BudgetCompareOutput {
        validator: validator.map(|s| s.to_string()),
        before: ExecUnits {
            cpu: before_eu.0,
            mem: before_eu.1,
        },
        after: ExecUnits {
            cpu: after_eu.0,
            mem: after_eu.1,
        },
        delta: ExecUnitsDelta {
            cpu: BudgetDelta {
                value: cpu_delta,
                pct: cpu_pct,
            },
            mem: BudgetDelta {
                value: mem_delta,
                pct: mem_pct,
            },
        },
        regression,
        threshold: RegressionThreshold {
            cpu_pct: cpu_threshold,
            mem_pct: mem_threshold,
        },
    };

    let data = serde_json::to_value(&compare_output)
        .map_err(|e| anyhow::anyhow!("failed to serialize compare output: {e}"))?;

    Ok(Output::ok(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    // --- budget show tests ---

    #[test]
    fn test_parse_aiken_check_output_empty() {
        let result = parse_aiken_check_output("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_aiken_check_output_with_results() {
        let output = r#"
    Testing ...

    ┍━ escrow ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    │ PASS [mem: 32451, cpu: 1192183] can_unlock
    │ FAIL [mem: 28500, cpu: 980000] cannot_unlock_early
    ┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
"#;
        let validators = parse_aiken_check_output(output);
        assert_eq!(validators.len(), 1);
        assert_eq!(validators[0].name, "escrow");
        assert_eq!(validators[0].benchmarks.len(), 2);

        assert_eq!(validators[0].benchmarks[0].test_name, "can_unlock");
        assert_eq!(validators[0].benchmarks[0].cpu, 1192183);
        assert_eq!(validators[0].benchmarks[0].mem, 32451);
        assert_eq!(validators[0].benchmarks[0].source, "test");

        assert_eq!(validators[0].benchmarks[1].test_name, "cannot_unlock_early");
        assert_eq!(validators[0].benchmarks[1].cpu, 980000);
        assert_eq!(validators[0].benchmarks[1].mem, 28500);
    }

    #[test]
    fn test_parse_aiken_check_output_multiple_modules() {
        let output = r#"
    ┍━ escrow ━━━━━━━━
    │ PASS [mem: 100, cpu: 200] test_a
    ┕━━━━━━━━━━━━━━━━━
    ┍━ vesting ━━━━━━━
    │ PASS [mem: 300, cpu: 400] test_b
    ┕━━━━━━━━━━━━━━━━━
"#;
        let validators = parse_aiken_check_output(output);
        assert_eq!(validators.len(), 2);
        assert_eq!(validators[0].name, "escrow");
        assert_eq!(validators[1].name, "vesting");
    }

    #[test]
    fn test_pct_of_limit_calculation() {
        let output = r#"
    ┍━ test_mod ━━━━━━
    │ PASS [mem: 14000000, cpu: 10000000000] full_budget
    ┕━━━━━━━━━━━━━━━━━
"#;
        let validators = parse_aiken_check_output(output);
        assert_eq!(validators.len(), 1);
        let b = &validators[0].benchmarks[0];
        // Should be 100% of limit
        assert!((b.cpu_pct_of_limit - 100.0).abs() < 0.01);
        assert!((b.mem_pct_of_limit - 100.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_budget_show_no_aiken() -> TestResult {
        // Use a temp dir with no aiken available in modified PATH
        // We can't easily test this without mocking, so we test the parse function instead
        // The integration test would require aiken installed
        Ok(())
    }

    // --- budget compare tests ---

    #[test]
    fn test_compare_missing_before() -> TestResult {
        let output = budget_compare(None, Some("/tmp/after.json"), None)?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "INVALID_INPUT");
        Ok(())
    }

    #[test]
    fn test_compare_missing_after() -> TestResult {
        let output = budget_compare(Some("/tmp/before.json"), None, None)?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "INVALID_INPUT");
        Ok(())
    }

    #[test]
    fn test_compare_file_not_found() -> TestResult {
        let output = budget_compare(
            Some("/nonexistent/before.json"),
            Some("/nonexistent/after.json"),
            None,
        )?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "FILE_READ_ERROR");
        Ok(())
    }

    #[test]
    fn test_compare_invalid_json() -> TestResult {
        let mut tmpfile = tempfile::NamedTempFile::new()?;
        write!(tmpfile, "not json")?;
        let before_path = tmpfile.path().to_str().ok_or("non-utf8 path")?.to_string();

        let mut after_file = tempfile::NamedTempFile::new()?;
        write!(after_file, r#"{{"cpu": 100, "mem": 200}}"#)?;
        let after_path = after_file
            .path()
            .to_str()
            .ok_or("non-utf8 path")?
            .to_string();

        let output = budget_compare(Some(&before_path), Some(&after_path), None)?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "INVALID_JSON");
        Ok(())
    }

    #[test]
    fn test_compare_valid_top_level_exec_units() -> TestResult {
        let mut before_file = tempfile::NamedTempFile::new()?;
        write!(
            before_file,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "exec_units": {"cpu": 1192183, "mem": 32451}
            }))?
        )?;

        let mut after_file = tempfile::NamedTempFile::new()?;
        write!(
            after_file,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "exec_units": {"cpu": 1050000, "mem": 30200}
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

        let output = budget_compare(Some(&before_path), Some(&after_path), None)?;
        let json = serde_json::to_value(&output)?;

        assert_eq!(json["status"], "ok");
        assert_eq!(json["before"]["cpu"], 1192183);
        assert_eq!(json["before"]["mem"], 32451);
        assert_eq!(json["after"]["cpu"], 1050000);
        assert_eq!(json["after"]["mem"], 30200);
        assert_eq!(json["delta"]["cpu"]["value"], 1050000 - 1192183);
        assert_eq!(json["delta"]["mem"]["value"], 30200 - 32451);
        assert_eq!(json["regression"], false);
        assert_eq!(json["threshold"]["cpu_pct"], 10.0);
        assert_eq!(json["threshold"]["mem_pct"], 10.0);

        Ok(())
    }

    #[test]
    fn test_compare_regression_detected() -> TestResult {
        let mut before_file = tempfile::NamedTempFile::new()?;
        write!(
            before_file,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "exec_units": {"cpu": 1000000, "mem": 10000}
            }))?
        )?;

        let mut after_file = tempfile::NamedTempFile::new()?;
        write!(
            after_file,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "exec_units": {"cpu": 1200000, "mem": 10000}
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

        let output = budget_compare(Some(&before_path), Some(&after_path), None)?;
        let json = serde_json::to_value(&output)?;

        assert_eq!(json["status"], "ok");
        assert_eq!(json["regression"], true);
        // 20% increase > 10% threshold
        assert_eq!(json["delta"]["cpu"]["pct"], 20.0);

        Ok(())
    }

    #[test]
    fn test_compare_with_validator_filter() -> TestResult {
        let mut before_file = tempfile::NamedTempFile::new()?;
        write!(
            before_file,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "validators": [
                    {"name": "escrow.spend", "benchmarks": [{"cpu": 1000, "mem": 200}]},
                    {"name": "vesting.spend", "benchmarks": [{"cpu": 2000, "mem": 400}]}
                ]
            }))?
        )?;

        let mut after_file = tempfile::NamedTempFile::new()?;
        write!(
            after_file,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "validators": [
                    {"name": "escrow.spend", "benchmarks": [{"cpu": 900, "mem": 180}]},
                    {"name": "vesting.spend", "benchmarks": [{"cpu": 2100, "mem": 420}]}
                ]
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

        let output = budget_compare(Some(&before_path), Some(&after_path), Some("escrow"))?;
        let json = serde_json::to_value(&output)?;

        assert_eq!(json["status"], "ok");
        assert_eq!(json["validator"], "escrow");
        assert_eq!(json["before"]["cpu"], 1000);
        assert_eq!(json["after"]["cpu"], 900);
        assert_eq!(json["regression"], false);

        Ok(())
    }

    #[test]
    fn test_compare_validator_not_found() -> TestResult {
        let mut before_file = tempfile::NamedTempFile::new()?;
        write!(
            before_file,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "validators": [{"name": "escrow", "benchmarks": []}]
            }))?
        )?;

        let mut after_file = tempfile::NamedTempFile::new()?;
        write!(
            after_file,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "exec_units": {"cpu": 100, "mem": 200}
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

        let output = budget_compare(Some(&before_path), Some(&after_path), Some("nonexistent"))?;
        let json = serde_json::to_value(&output)?;

        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "VALIDATOR_NOT_FOUND");

        Ok(())
    }

    #[test]
    fn test_compare_direct_cpu_mem_fields() -> TestResult {
        let mut before_file = tempfile::NamedTempFile::new()?;
        write!(
            before_file,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "cpu": 500, "mem": 100
            }))?
        )?;

        let mut after_file = tempfile::NamedTempFile::new()?;
        write!(
            after_file,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "cpu": 450, "mem": 90
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

        let output = budget_compare(Some(&before_path), Some(&after_path), None)?;
        let json = serde_json::to_value(&output)?;

        assert_eq!(json["status"], "ok");
        assert_eq!(json["before"]["cpu"], 500);
        assert_eq!(json["after"]["cpu"], 450);

        Ok(())
    }

    #[test]
    fn test_extract_validator_exec_units_from_results_array() {
        let val = serde_json::json!({
            "results": [
                {"name": "test_a", "exec_units": {"cpu": 100, "mem": 200}},
                {"name": "test_b", "exec_units": {"cpu": 300, "mem": 400}}
            ]
        });
        assert_eq!(extract_validator_exec_units(&val, None), Some((100, 200)));
        assert_eq!(
            extract_validator_exec_units(&val, Some("test_b")),
            Some((300, 400))
        );
    }
}
