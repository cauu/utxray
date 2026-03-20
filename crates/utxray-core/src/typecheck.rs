use crate::aiken::cli::AikenCli;
use crate::build::parse_aiken_errors;
use crate::output::Output;

/// Run `aiken check` and produce structured output.
///
/// On success, returns checked module count and warnings.
/// On failure, returns structured error information.
pub async fn run_typecheck(
    project_dir: &str,
    module: Option<&str>,
    trace_level: &str,
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

    let start = std::time::Instant::now();
    let aiken_out = cli.check(module, trace_level).await?;
    let elapsed_ms = start.elapsed().as_millis();

    if aiken_out.exit_code != 0 {
        let errors = parse_aiken_errors(&aiken_out.raw_stderr);
        let output = Output::error(serde_json::json!({
            "errors": errors,
            "check_time_ms": elapsed_ms
        }));
        return Ok(output);
    }

    // Parse stdout for module count. Aiken check output varies, so we
    // count lines mentioning modules or just report a count of 1 if we
    // can't determine the exact number.
    let checked_modules = count_checked_modules(&aiken_out.raw_stdout, &aiken_out.raw_stderr);
    let warnings = extract_warnings(&aiken_out.raw_stderr);

    let data = serde_json::json!({
        "checked_modules": checked_modules,
        "warnings": warnings,
        "check_time_ms": elapsed_ms
    });

    Ok(Output::ok(data))
}

/// Count the number of checked modules from aiken check output.
fn count_checked_modules(stdout: &str, stderr: &str) -> usize {
    // Aiken outputs something like "Compiling X module(s)" or similar.
    // We try to extract the number; otherwise default to counting non-empty relevant lines.
    let combined = format!("{stdout}\n{stderr}");

    for line in combined.lines() {
        let lower = line.to_lowercase();
        // Look for patterns like "Compiling 3 module(s)" or "Checking 3 module(s)"
        if lower.contains("module") {
            let words: Vec<&str> = line.split_whitespace().collect();
            for (i, word) in words.iter().enumerate() {
                if let Ok(n) = word.parse::<usize>() {
                    // Verify next word contains "module"
                    if i + 1 < words.len() && words[i + 1].to_lowercase().contains("module") {
                        return n;
                    }
                }
            }
        }
    }

    // Fallback: if build succeeded, at least 1 module was checked
    1
}

/// Extract warning strings from aiken check stderr.
fn extract_warnings(stderr: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    for line in stderr.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();
        if lower.contains("warning") && !lower.contains("error") {
            warnings.push(trimmed.to_string());
        }
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_checked_modules_with_number() {
        assert_eq!(count_checked_modules("Compiling 3 modules", ""), 3);
    }

    #[test]
    fn test_count_checked_modules_fallback() {
        assert_eq!(count_checked_modules("done", ""), 1);
    }

    #[test]
    fn test_extract_warnings_empty() {
        let warnings = extract_warnings("");
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_extract_warnings_with_content() {
        let stderr = "Warning: unused variable\nError: something\n";
        let warnings = extract_warnings(stderr);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Warning"));
    }
}
