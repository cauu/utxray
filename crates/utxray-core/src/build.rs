use std::path::Path;

use serde::Serialize;

use crate::aiken::cli::AikenCli;
use crate::output::Output;

/// A single validator entry extracted from plutus.json blueprint.
#[derive(Debug, Clone, Serialize)]
pub struct ValidatorInfo {
    pub name: String,
    pub purpose: String,
    pub hash: String,
    pub plutus_version: String,
    pub size_bytes: usize,
}

/// Successful build output data.
#[derive(Debug, Serialize)]
pub struct BuildSuccess {
    pub validators: Vec<ValidatorInfo>,
    pub blueprint_path: String,
    pub compile_time_ms: u128,
}

/// A structured compile error.
#[derive(Debug, Clone, Serialize)]
pub struct CompileError {
    pub severity: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub col: Option<u64>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

/// Build error output data.
#[derive(Debug, Serialize)]
pub struct BuildError {
    pub errors: Vec<CompileError>,
}

/// Run `aiken build` and produce structured output.
///
/// On success, reads `plutus.json` from the project directory and extracts
/// validator information. On failure, parses stderr for error details.
pub async fn run_build(project_dir: &str) -> anyhow::Result<Output<serde_json::Value>> {
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
    let aiken_out = cli.build().await?;
    let compile_time_ms = start.elapsed().as_millis();

    if aiken_out.exit_code != 0 {
        let errors = parse_aiken_errors(&aiken_out.raw_stderr);
        let output = Output::error(serde_json::json!({
            "errors": errors,
            "compile_time_ms": compile_time_ms
        }));
        return Ok(output);
    }

    // Build succeeded — read plutus.json
    let blueprint_path = Path::new(project_dir).join("plutus.json");
    let validators = if blueprint_path.exists() {
        let content = std::fs::read_to_string(&blueprint_path)
            .map_err(|e| anyhow::anyhow!("failed to read plutus.json: {e}"))?;
        parse_blueprint(&content)?
    } else {
        vec![]
    };

    let relative_blueprint = "./plutus.json".to_string();

    let data = serde_json::json!({
        "validators": validators.iter().map(|v| serde_json::json!({
            "name": v.name,
            "purpose": v.purpose,
            "hash": v.hash,
            "plutus_version": v.plutus_version,
            "size_bytes": v.size_bytes,
        })).collect::<Vec<_>>(),
        "blueprint_path": relative_blueprint,
        "compile_time_ms": compile_time_ms
    });

    Ok(Output::ok(data))
}

/// Parse aiken stderr output into structured error objects.
pub fn parse_aiken_errors(stderr: &str) -> Vec<CompileError> {
    if stderr.trim().is_empty() {
        return vec![CompileError {
            severity: "critical".to_string(),
            code: "COMPILE_ERROR".to_string(),
            file: None,
            line: None,
            col: None,
            message: "aiken build failed with no error output".to_string(),
            snippet: None,
            hint: None,
        }];
    }

    // Aiken error output format varies. We do a best-effort parse.
    // Typical aiken error lines look like:
    //   Error in module_name:
    //   ┌─ validators/foo.ak:10:5
    //   │ ...
    //   = hint: ...
    let mut errors = Vec::new();
    let mut current_message = String::new();
    let mut current_file: Option<String> = None;
    let mut current_line: Option<u64> = None;
    let mut current_col: Option<u64> = None;
    let mut current_hint: Option<String> = None;
    let mut current_snippet: Option<String> = None;

    for line in stderr.lines() {
        let trimmed = line.trim();

        // Try to match file:line:col pattern like "┌─ validators/foo.ak:10:5"
        if let Some(loc) = trimmed.strip_prefix("┌─") {
            let loc = loc.trim();
            let parts: Vec<&str> = loc.rsplitn(3, ':').collect();
            if parts.len() >= 3 {
                current_col = parts[0].parse().ok();
                current_line = parts[1].parse().ok();
                current_file = Some(parts[2].to_string());
            } else if parts.len() == 2 {
                current_line = parts[0].parse().ok();
                current_file = Some(parts[1].to_string());
            }
        } else if let Some(hint) = trimmed.strip_prefix("= ") {
            current_hint = Some(hint.to_string());
        } else if trimmed.starts_with("│") {
            let snippet_line = trimmed.strip_prefix("│").unwrap_or(trimmed).trim();
            if !snippet_line.is_empty() {
                match &mut current_snippet {
                    Some(s) => {
                        s.push('\n');
                        s.push_str(snippet_line);
                    }
                    None => current_snippet = Some(snippet_line.to_string()),
                }
            }
        } else if trimmed.starts_with("Error") || trimmed.starts_with("error") {
            // Flush previous error if any
            if !current_message.is_empty() {
                errors.push(CompileError {
                    severity: "critical".to_string(),
                    code: "COMPILE_ERROR".to_string(),
                    file: current_file.take(),
                    line: current_line.take(),
                    col: current_col.take(),
                    message: current_message.clone(),
                    snippet: current_snippet.take(),
                    hint: current_hint.take(),
                });
            }
            current_message = trimmed.to_string();
        } else if !trimmed.is_empty() {
            if !current_message.is_empty() {
                current_message.push(' ');
            }
            current_message.push_str(trimmed);
        }
    }

    // Flush last accumulated error
    if !current_message.is_empty() {
        errors.push(CompileError {
            severity: "critical".to_string(),
            code: "COMPILE_ERROR".to_string(),
            file: current_file,
            line: current_line,
            col: current_col,
            message: current_message,
            snippet: current_snippet,
            hint: current_hint,
        });
    }

    if errors.is_empty() {
        // Fallback: put the entire stderr as one error
        errors.push(CompileError {
            severity: "critical".to_string(),
            code: "COMPILE_ERROR".to_string(),
            file: None,
            line: None,
            col: None,
            message: stderr.trim().to_string(),
            snippet: None,
            hint: None,
        });
    }

    errors
}

/// Parse a CIP-0057 blueprint (plutus.json) and extract validator info.
pub fn parse_blueprint(content: &str) -> anyhow::Result<Vec<ValidatorInfo>> {
    let blueprint: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| anyhow::anyhow!("failed to parse plutus.json: {e}"))?;

    let mut validators = Vec::new();

    let empty_vec = vec![];
    let validator_array = blueprint
        .get("validators")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_vec);

    for v in validator_array {
        let title = v.get("title").and_then(|t| t.as_str()).unwrap_or("unknown");

        // Parse name and purpose from title (format: "module.function")
        let (name, purpose) = parse_validator_title(title);

        let hash = v
            .get("hash")
            .and_then(|h| h.as_str())
            .unwrap_or("")
            .to_string();

        // Detect plutus version from compiledCode prefix or preamble
        let plutus_version = detect_plutus_version(&blueprint, v);

        // Calculate size from compiled code (hex-encoded CBOR)
        let size_bytes = v
            .get("compiledCode")
            .and_then(|c| c.as_str())
            .map(|c| c.len() / 2) // hex -> bytes
            .unwrap_or(0);

        validators.push(ValidatorInfo {
            name,
            purpose,
            hash,
            plutus_version,
            size_bytes,
        });
    }

    Ok(validators)
}

/// Parse a validator title like "module.function" into (name, purpose).
fn parse_validator_title(title: &str) -> (String, String) {
    // Aiken blueprints use titles like "module_name.validator_name"
    // The purpose is typically embedded or inferred
    let parts: Vec<&str> = title.splitn(2, '.').collect();
    if parts.len() == 2 {
        (parts[1].to_string(), infer_purpose(parts[1]))
    } else {
        (title.to_string(), "spend".to_string())
    }
}

/// Infer a validator's purpose from its name. This is a heuristic.
fn infer_purpose(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("mint") || lower.contains("policy") {
        "mint".to_string()
    } else if lower.contains("withdraw") {
        "withdraw".to_string()
    } else if lower.contains("publish") || lower.contains("cert") {
        "publish".to_string()
    } else {
        "spend".to_string()
    }
}

/// Detect the Plutus version from the blueprint preamble or validator data.
fn detect_plutus_version(blueprint: &serde_json::Value, _validator: &serde_json::Value) -> String {
    // CIP-0057 blueprints have a preamble.plutusVersion field
    if let Some(preamble) = blueprint.get("preamble") {
        if let Some(version) = preamble.get("plutusVersion") {
            if let Some(v) = version.as_str() {
                return v.to_string();
            }
        }
    }
    // Default to v3 for modern Aiken
    "v3".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_parse_blueprint_empty_validators() -> TestResult {
        let json = r#"{"preamble": {"plutusVersion": "v3"}, "validators": []}"#;
        let result = parse_blueprint(json)?;
        assert!(result.is_empty());
        Ok(())
    }

    #[test]
    fn test_parse_blueprint_with_validator() -> TestResult {
        let json = r#"{
            "preamble": {"plutusVersion": "v3"},
            "validators": [{
                "title": "hello_world.spend",
                "hash": "abc123",
                "compiledCode": "aabbccdd"
            }]
        }"#;
        let result = parse_blueprint(json)?;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "spend");
        assert_eq!(result[0].purpose, "spend");
        assert_eq!(result[0].hash, "abc123");
        assert_eq!(result[0].plutus_version, "v3");
        assert_eq!(result[0].size_bytes, 4); // 8 hex chars / 2
        Ok(())
    }

    #[test]
    fn test_parse_blueprint_invalid_json() {
        let result = parse_blueprint("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_aiken_errors_empty() {
        let errors = parse_aiken_errors("");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "COMPILE_ERROR");
    }

    #[test]
    fn test_parse_aiken_errors_simple() {
        let stderr = "Error in module:\n  something went wrong\n";
        let errors = parse_aiken_errors(stderr);
        assert!(!errors.is_empty());
        assert!(errors[0].message.contains("Error"));
    }

    #[test]
    fn test_parse_validator_title() {
        let (name, purpose) = parse_validator_title("my_module.my_validator");
        assert_eq!(name, "my_validator");
        assert_eq!(purpose, "spend");

        let (name, purpose) = parse_validator_title("my_module.mint_token");
        assert_eq!(name, "mint_token");
        assert_eq!(purpose, "mint");
    }

    #[test]
    fn test_detect_plutus_version() {
        let bp = serde_json::json!({"preamble": {"plutusVersion": "v2"}});
        let v = serde_json::json!({});
        assert_eq!(detect_plutus_version(&bp, &v), "v2");

        let bp_no_version = serde_json::json!({});
        assert_eq!(detect_plutus_version(&bp_no_version, &v), "v3");
    }

    #[test]
    fn test_infer_purpose() {
        assert_eq!(infer_purpose("spend_validator"), "spend");
        assert_eq!(infer_purpose("mint_policy"), "mint");
        assert_eq!(infer_purpose("withdraw_reward"), "withdraw");
        assert_eq!(infer_purpose("publish_cert"), "publish");
        assert_eq!(infer_purpose("my_validator"), "spend");
    }
}
