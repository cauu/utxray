use serde::Serialize;

use crate::aiken::cli::AikenCli;
use crate::output::{Output, Status};

/// A single test result entry.
#[derive(Debug, Clone, Serialize)]
pub struct TestResult {
    pub name: String,
    pub module: String,
    pub result: String,
    pub exec_units: ExecUnits,
    pub budget_source: String,
    pub traces: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_detail: Option<String>,
}

/// Execution units for a test.
#[derive(Debug, Clone, Serialize)]
pub struct ExecUnits {
    pub cpu: u64,
    pub mem: u64,
}

/// Summary of test results.
#[derive(Debug, Clone, Serialize)]
pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

/// Full test output data.
#[derive(Debug, Serialize)]
pub struct TestOutput {
    pub summary: TestSummary,
    pub results: Vec<TestResult>,
}

/// Run Aiken tests and produce structured output.
pub async fn run_test(
    project_dir: &str,
    match_pattern: Option<&str>,
    module: Option<&str>,
    trace_level: &str,
    seed: Option<u64>,
) -> anyhow::Result<Output<serde_json::Value>> {
    let cli = match AikenCli::new(project_dir) {
        Ok(c) => c,
        Err(e) => {
            let output = Output::error(serde_json::json!({
                "errors": [{
                    "severity": "critical",
                    "code": "AIKEN_NOT_FOUND",
                    "message": e.to_string()
                }]
            }));
            return Ok(output);
        }
    };

    let aiken_out = cli.test(match_pattern, module, trace_level, seed).await?;

    // Combine stdout and stderr for parsing (aiken may write to either)
    let combined = format!("{}\n{}", aiken_out.raw_stdout, aiken_out.raw_stderr);
    let results = parse_test_output(&combined);

    // If no results were parsed and exit code is non-zero, this is a compile/tool error
    if results.is_empty() && aiken_out.exit_code != 0 {
        let errors = crate::build::parse_aiken_errors(&aiken_out.raw_stderr);
        let output = Output::error(serde_json::json!({
            "errors": errors,
        }));
        return Ok(output);
    }

    let passed = results.iter().filter(|r| r.result == "pass").count();
    let failed = results.iter().filter(|r| r.result == "fail").count();
    let total = results.len();

    let summary = TestSummary {
        total,
        passed,
        failed,
    };

    let data = serde_json::json!({
        "summary": summary,
        "results": results,
    });

    let status = if total == 0 {
        // No tests found — tool ran fine but nothing to report
        Status::Ok
    } else if failed == 0 {
        Status::Ok
    } else if passed == 0 {
        // All failed — still "mixed" per spec (tool ran fine, tests failed)
        Status::Mixed
    } else {
        Status::Mixed
    };

    let output = Output {
        v: crate::output::UTXRAY_VERSION.to_string(),
        status,
        data,
        warnings: vec![],
    };

    Ok(output)
}

/// Parse the text output of `aiken check` to extract test results.
///
/// Expected format:
/// ```text
/// ┍━ validators/escrow ━━━━━━━━━━━
/// │ PASS [mem: 32451, cpu: 1192183] can_unlock_with_correct_signature
/// │ ↳ checking redeemer type: Unlock
/// │ FAIL [mem: 28100, cpu: 892401] cannot_unlock_after_deadline
/// │ ↳ FAIL — current_slot 300 > deadline 200
/// ┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
/// ```
/// Try to parse aiken's structured JSON test output (aiken v1.1+).
/// Returns None if the output isn't valid aiken JSON.
fn try_parse_json_test_output(output: &str) -> Option<Vec<TestResult>> {
    // Find JSON object in the output (aiken may print compilation messages before it)
    let json_start = output.find('{')?;
    let json_str = &output[json_start..];

    // The output may have trailing non-JSON text (e.g. stderr appended after stdout).
    // Find the matching closing brace to extract only the JSON portion.
    let mut depth = 0i32;
    let mut json_end = json_str.len();
    for (i, c) in json_str.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    json_end = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    let clean_json = &json_str[..json_end];
    let parsed: serde_json::Value = serde_json::from_str(clean_json).ok()?;

    let modules = parsed.get("modules")?.as_array()?;
    let mut results = Vec::new();

    for module in modules {
        let module_name = module.get("name")?.as_str().unwrap_or("unknown");
        let tests = module.get("tests")?.as_array()?;

        for test in tests {
            let title = test.get("title")?.as_str().unwrap_or("unknown");
            let status = test.get("status")?.as_str().unwrap_or("unknown");
            let eu = test
                .get("execution_units")
                .unwrap_or(&serde_json::Value::Null);
            let cpu = eu.get("cpu").and_then(|v| v.as_u64()).unwrap_or(0);
            let mem = eu.get("mem").and_then(|v| v.as_u64()).unwrap_or(0);

            let result_str = if status == "pass" { "pass" } else { "fail" };
            let error_detail = if status != "pass" {
                test.get("error")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            };

            results.push(TestResult {
                name: title.to_string(),
                module: module_name.to_string(),
                result: result_str.to_string(),
                exec_units: ExecUnits { cpu, mem },
                budget_source: "test".to_string(),
                traces: vec![],
                error_detail,
            });
        }
    }

    if results.is_empty() {
        None
    } else {
        Some(results)
    }
}

pub fn parse_test_output(output: &str) -> Vec<TestResult> {
    // Try JSON parsing first (aiken v1.1+), fall back to box-drawing text
    if let Some(results) = try_parse_json_test_output(output) {
        return results;
    }

    let mut results = Vec::new();
    let mut current_module = String::new();
    let mut current_result: Option<TestResult> = None;

    for line in output.lines() {
        let trimmed = line.trim();

        // Module header: "┍━ validators/escrow ━━━"
        if let Some(rest) = trimmed.strip_prefix("┍━") {
            // Flush previous result
            if let Some(r) = current_result.take() {
                results.push(r);
            }
            // Extract module name by stripping trailing box chars
            let module_part = rest.trim().trim_end_matches('━').trim();
            current_module = module_part.to_string();
            continue;
        }

        // Section end
        if trimmed.starts_with("┕") {
            if let Some(r) = current_result.take() {
                results.push(r);
            }
            continue;
        }

        // Strip leading "│ " for content lines
        let content = if let Some(rest) = trimmed.strip_prefix('│') {
            rest.trim()
        } else {
            continue;
        };

        // Trace line: "↳ some trace message"
        if let Some(trace_msg) = content.strip_prefix('↳') {
            if let Some(ref mut r) = current_result {
                r.traces.push(trace_msg.trim().to_string());
                // If this is a FAIL trace, also capture as error_detail
                if trace_msg.trim().starts_with("FAIL") {
                    r.error_detail = Some(trace_msg.trim().to_string());
                }
            }
            continue;
        }

        // Test result line: "PASS [mem: 32451, cpu: 1192183] test_name"
        //                 or "FAIL [mem: 28100, cpu: 892401] test_name"
        if content.starts_with("PASS") || content.starts_with("FAIL") {
            // Flush previous result
            if let Some(r) = current_result.take() {
                results.push(r);
            }

            let is_pass = content.starts_with("PASS");
            let (exec_units, name) = parse_result_line(content);

            current_result = Some(TestResult {
                name,
                module: current_module.clone(),
                result: if is_pass {
                    "pass".to_string()
                } else {
                    "fail".to_string()
                },
                exec_units,
                budget_source: "test".to_string(),
                traces: Vec::new(),
                error_detail: if is_pass {
                    None
                } else {
                    Some("Test returned False".to_string())
                },
            });
        }
    }

    // Flush last result
    if let Some(r) = current_result.take() {
        results.push(r);
    }

    results
}

/// Parse a PASS/FAIL line to extract exec units and test name.
///
/// Format: "PASS [mem: 32451, cpu: 1192183] test_name"
/// or:     "FAIL [mem: 28100, cpu: 892401] test_name"
fn parse_result_line(line: &str) -> (ExecUnits, String) {
    let default_units = ExecUnits { cpu: 0, mem: 0 };

    // Find the bracket section
    let bracket_start = match line.find('[') {
        Some(i) => i,
        None => {
            // No brackets — just extract name after PASS/FAIL
            let name = line
                .strip_prefix("PASS")
                .or_else(|| line.strip_prefix("FAIL"))
                .unwrap_or(line)
                .trim()
                .to_string();
            return (default_units, name);
        }
    };

    let bracket_end = match line.find(']') {
        Some(i) => i,
        None => {
            let name = line[bracket_start..].trim().to_string();
            return (default_units, name);
        }
    };

    let bracket_content = &line[bracket_start + 1..bracket_end];
    let name = line[bracket_end + 1..].trim().to_string();

    let mut cpu: u64 = 0;
    let mut mem: u64 = 0;

    for part in bracket_content.split(',') {
        let part = part.trim();
        if let Some(val) = part.strip_prefix("mem:") {
            mem = val.trim().parse().unwrap_or(0);
        } else if let Some(val) = part.strip_prefix("cpu:") {
            cpu = val.trim().parse().unwrap_or(0);
        }
    }

    (ExecUnits { cpu, mem }, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_result_line_pass() {
        let (units, name) = parse_result_line("PASS [mem: 32451, cpu: 1192183] can_unlock");
        assert_eq!(name, "can_unlock");
        assert_eq!(units.cpu, 1192183);
        assert_eq!(units.mem, 32451);
    }

    #[test]
    fn test_parse_result_line_fail() {
        let (units, name) =
            parse_result_line("FAIL [mem: 28100, cpu: 892401] cannot_unlock_after_deadline");
        assert_eq!(name, "cannot_unlock_after_deadline");
        assert_eq!(units.cpu, 892401);
        assert_eq!(units.mem, 28100);
    }

    #[test]
    fn test_parse_result_line_no_brackets() {
        let (units, name) = parse_result_line("PASS some_test");
        assert_eq!(name, "some_test");
        assert_eq!(units.cpu, 0);
        assert_eq!(units.mem, 0);
    }

    #[test]
    fn test_parse_test_output_mixed() {
        let output = r#"
    Compiling aiken-lang/stdlib v2.2.0
    Compiling my-project/validators
      Testing ...

    ┍━ validators/escrow ━━━━━━━━━━━
    │ PASS [mem: 32451, cpu: 1192183] can_unlock_with_correct_signature
    │ ↳ checking redeemer type: Unlock
    │ ↳ verifying signature: ok
    │ FAIL [mem: 28100, cpu: 892401] cannot_unlock_after_deadline
    │ ↳ checking redeemer type: Unlock
    │ ↳ FAIL — current_slot 300 > deadline 200
    ┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
"#;
        let results = parse_test_output(output);
        assert_eq!(results.len(), 2);

        assert_eq!(results[0].name, "can_unlock_with_correct_signature");
        assert_eq!(results[0].module, "validators/escrow");
        assert_eq!(results[0].result, "pass");
        assert_eq!(results[0].exec_units.cpu, 1192183);
        assert_eq!(results[0].exec_units.mem, 32451);
        assert_eq!(results[0].traces.len(), 2);
        assert_eq!(results[0].traces[0], "checking redeemer type: Unlock");
        assert_eq!(results[0].traces[1], "verifying signature: ok");
        assert!(results[0].error_detail.is_none());

        assert_eq!(results[1].name, "cannot_unlock_after_deadline");
        assert_eq!(results[1].module, "validators/escrow");
        assert_eq!(results[1].result, "fail");
        assert_eq!(results[1].exec_units.cpu, 892401);
        assert_eq!(results[1].exec_units.mem, 28100);
        assert_eq!(results[1].traces.len(), 2);
        // error_detail should be updated to the FAIL trace
        assert!(results[1]
            .error_detail
            .as_ref()
            .is_some_and(|d| d.contains("FAIL")));
    }

    #[test]
    fn test_parse_test_output_all_pass() {
        let output = r#"
    ┍━ validators/always_true ━━━━━━
    │ PASS [mem: 1000, cpu: 5000] test_always_succeeds
    ┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
"#;
        let results = parse_test_output(output);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result, "pass");
        assert_eq!(results[0].module, "validators/always_true");
    }

    #[test]
    fn test_parse_test_output_empty() {
        let results = parse_test_output("Some compiling output\nDone.\n");
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_test_output_multiple_modules() {
        let output = r#"
    ┍━ validators/escrow ━━━━━━━━━━━
    │ PASS [mem: 100, cpu: 200] test_a
    ┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    ┍━ validators/mint ━━━━━━━━━━━━━
    │ FAIL [mem: 300, cpu: 400] test_b
    │ ↳ some trace
    ┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
"#;
        let results = parse_test_output(output);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].module, "validators/escrow");
        assert_eq!(results[0].name, "test_a");
        assert_eq!(results[1].module, "validators/mint");
        assert_eq!(results[1].name, "test_b");
    }

    #[test]
    fn test_parse_result_line_large_numbers() {
        let (units, name) = parse_result_line("PASS [mem: 999999999, cpu: 14000000000] big_test");
        assert_eq!(name, "big_test");
        assert_eq!(units.cpu, 14000000000);
        assert_eq!(units.mem, 999999999);
    }
}
