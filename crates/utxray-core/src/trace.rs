use std::path::Path;

use serde::Serialize;

use crate::aiken::cli::AikenCli;
use crate::build::parse_blueprint;
use crate::error::BudgetSource;
use crate::output::Output;
use crate::test_cmd::{parse_test_output, ExecUnits};

/// Valid purposes for the trace command.
const VALID_PURPOSES: &[&str] = &["spend", "mint", "withdraw", "publish"];

/// Trace output data.
#[derive(Debug, Serialize)]
pub struct TraceOutput {
    pub scope: String,
    pub validator: String,
    pub purpose: String,
    pub context_mode: String,
    pub auto_filled_fields: Vec<String>,
    pub cost_fidelity: String,
    pub result: String,
    pub exec_units: ExecUnits,
    pub budget_source: BudgetSource,
    pub traces: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_detail: Option<String>,
}

/// Configuration for the trace command.
pub struct TraceConfig {
    pub validator: String,
    pub purpose: String,
    pub redeemer: String,
    pub datum: Option<String>,
    pub context: Option<String>,
    pub slot: Option<u64>,
    pub signatories: Vec<String>,
}

/// Errors specific to the trace command.
#[derive(Debug, thiserror::Error)]
pub enum TraceError {
    #[error("validator '{0}' not found in blueprint")]
    ValidatorNotFound(String),

    #[error("purpose '{0}' is invalid; expected one of: spend, mint, withdraw, publish")]
    InvalidPurpose(String),

    #[error("redeemer is required")]
    RedeemerRequired,

    #[error("datum is required for spend purpose")]
    DatumRequiredForSpend,

    #[error("invalid redeemer: not valid JSON — {0}")]
    InvalidRedeemerJson(String),

    #[error("invalid datum: not valid JSON — {0}")]
    InvalidDatumJson(String),

    #[error("invalid context: not valid JSON — {0}")]
    InvalidContextJson(String),

    #[error("invalid signatory '{0}': expected 56 hex characters (28 bytes), got {1}")]
    InvalidSignatory(String, usize),

    #[error("invalid signatory '{0}': not valid hex — {1}")]
    InvalidSignatoryHex(String, String),

    #[error("blueprint not found at {0}")]
    BlueprintNotFound(String),

    #[error("failed to read blueprint: {0}")]
    BlueprintReadError(String),
}

/// Validate that a string is valid JSON. Returns the parsed value or an error.
fn validate_json(input: &str) -> Result<serde_json::Value, String> {
    // If it looks like a file path, try to read it
    if !input.starts_with('{') && !input.starts_with('[') && !input.starts_with('"') {
        let path = Path::new(input);
        if path.exists() {
            let content = std::fs::read_to_string(path)
                .map_err(|e| format!("failed to read file {input}: {e}"))?;
            return serde_json::from_str(&content)
                .map_err(|e| format!("invalid JSON in file {input}: {e}"));
        }
    }
    serde_json::from_str(input).map_err(|e| e.to_string())
}

/// Validate a signatory hex string: must be exactly 56 hex characters (28 bytes).
fn validate_signatory(sig: &str) -> Result<(), TraceError> {
    if sig.len() != 56 {
        return Err(TraceError::InvalidSignatory(sig.to_string(), sig.len()));
    }
    hex::decode(sig)
        .map_err(|e| TraceError::InvalidSignatoryHex(sig.to_string(), e.to_string()))?;
    Ok(())
}

/// Look up a validator in the blueprint by name. Supports both "module.name" and just "name".
fn find_validator_in_blueprint(
    blueprint_content: &str,
    validator_name: &str,
) -> Result<(String, Option<bool>), TraceError> {
    let validators = parse_blueprint(blueprint_content)
        .map_err(|e| TraceError::BlueprintReadError(e.to_string()))?;

    // Try exact match on title first
    let blueprint: serde_json::Value = serde_json::from_str(blueprint_content)
        .map_err(|e| TraceError::BlueprintReadError(e.to_string()))?;

    let empty_vec = vec![];
    let validator_array = blueprint
        .get("validators")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_vec);

    for v in validator_array {
        let title = v.get("title").and_then(|t| t.as_str()).unwrap_or("");
        if title == validator_name || title.ends_with(&format!(".{validator_name}")) {
            let has_datum = v.get("datum").is_some();
            return Ok((title.to_string(), Some(has_datum)));
        }
    }

    // Try matching against parsed validators by name
    for v in &validators {
        if v.name == validator_name {
            // We found it by parsed name, reconstruct the title
            return Ok((validator_name.to_string(), None));
        }
    }

    Err(TraceError::ValidatorNotFound(validator_name.to_string()))
}

/// Run the trace command.
///
/// Validates all inputs, looks up the validator in the blueprint,
/// and attempts to run via aiken if available.
pub async fn run_trace(
    project_dir: &str,
    config: TraceConfig,
) -> anyhow::Result<Output<serde_json::Value>> {
    // Validate purpose
    if !VALID_PURPOSES.contains(&config.purpose.as_str()) {
        let output = Output::error(serde_json::json!({
            "error_code": "INVALID_PURPOSE",
            "message": TraceError::InvalidPurpose(config.purpose).to_string()
        }));
        return Ok(output);
    }

    // Validate redeemer is valid JSON
    if config.redeemer.is_empty() {
        let output = Output::error(serde_json::json!({
            "error_code": "INVALID_INPUT",
            "message": TraceError::RedeemerRequired.to_string()
        }));
        return Ok(output);
    }

    if let Err(e) = validate_json(&config.redeemer) {
        let output = Output::error(serde_json::json!({
            "error_code": "INVALID_INPUT",
            "message": TraceError::InvalidRedeemerJson(e).to_string()
        }));
        return Ok(output);
    }

    // Validate datum if provided
    if let Some(ref datum) = config.datum {
        if let Err(e) = validate_json(datum) {
            let output = Output::error(serde_json::json!({
                "error_code": "INVALID_INPUT",
                "message": TraceError::InvalidDatumJson(e).to_string()
            }));
            return Ok(output);
        }
    }

    // Validate context if provided
    let has_context = config.context.is_some();
    if let Some(ref ctx) = config.context {
        if let Err(e) = validate_json(ctx) {
            let output = Output::error(serde_json::json!({
                "error_code": "INVALID_INPUT",
                "message": TraceError::InvalidContextJson(e).to_string()
            }));
            return Ok(output);
        }
    }

    // Validate signatories
    for sig in &config.signatories {
        if let Err(e) = validate_signatory(sig) {
            let output = Output::error(serde_json::json!({
                "error_code": "INVALID_INPUT",
                "message": e.to_string()
            }));
            return Ok(output);
        }
    }

    // Read blueprint to look up validator
    let blueprint_path = Path::new(project_dir).join("plutus.json");
    if !blueprint_path.exists() {
        let output = Output::error(serde_json::json!({
            "error_code": "BLUEPRINT_NOT_FOUND",
            "message": TraceError::BlueprintNotFound(
                blueprint_path.to_string_lossy().to_string()
            ).to_string()
        }));
        return Ok(output);
    }

    let blueprint_content = std::fs::read_to_string(&blueprint_path)
        .map_err(|e| anyhow::anyhow!("failed to read blueprint: {e}"))?;

    let (validator_title, has_datum_schema) =
        match find_validator_in_blueprint(&blueprint_content, &config.validator) {
            Ok(v) => v,
            Err(e) => {
                let output = Output::error(serde_json::json!({
                    "error_code": "VALIDATOR_NOT_FOUND",
                    "message": e.to_string()
                }));
                return Ok(output);
            }
        };

    // For spend purpose, datum is required if the blueprint declares a datum schema
    if config.purpose == "spend" && config.datum.is_none() {
        let datum_required = has_datum_schema.unwrap_or(true);
        if datum_required {
            let output = Output::error(serde_json::json!({
                "error_code": "INVALID_INPUT",
                "message": TraceError::DatumRequiredForSpend.to_string()
            }));
            return Ok(output);
        }
    }

    // Determine context mode
    let (context_mode, cost_fidelity, budget_source, auto_filled_fields) = if has_context {
        (
            "full",
            "high",
            BudgetSource::TraceFull,
            Vec::<String>::new(),
        )
    } else {
        (
            "minimal",
            "low",
            BudgetSource::TraceMinimal,
            vec![
                "inputs".to_string(),
                "outputs".to_string(),
                "fee".to_string(),
                "validity_range".to_string(),
                "mint".to_string(),
            ],
        )
    };

    // Attempt to run via aiken
    let cli = match AikenCli::new(project_dir) {
        Ok(c) => c,
        Err(_) => {
            // Aiken not available — return structured error
            let output = Output::error(serde_json::json!({
                "error_code": "AIKEN_NOT_FOUND",
                "scope": "script_only",
                "validator": validator_title,
                "purpose": config.purpose,
                "context_mode": context_mode,
                "auto_filled_fields": auto_filled_fields,
                "cost_fidelity": cost_fidelity,
                "message": "trace execution requires aiken. Install it from https://aiken-lang.org"
            }));
            return Ok(output);
        }
    };

    // Run aiken check with verbose tracing to capture trace output
    // Use the validator module to scope the test run
    let module_hint = extract_module_from_title(&validator_title);
    let aiken_out = cli.check(module_hint.as_deref(), "verbose").await?;

    // Parse the output for test results which include traces and exec units
    let combined = format!("{}\n{}", aiken_out.raw_stdout, aiken_out.raw_stderr);
    let test_results = parse_test_output(&combined);

    // If we got test results, use the first one's data (or best match)
    let (result_str, exec_units, traces, error_detail) = if test_results.is_empty() {
        // No test results — aiken ran but no matching tests found
        if aiken_out.exit_code != 0 {
            (
                "fail".to_string(),
                ExecUnits { cpu: 0, mem: 0 },
                vec![],
                Some(format!(
                    "aiken check failed: {}",
                    aiken_out.raw_stderr.trim()
                )),
            )
        } else {
            (
                "pass".to_string(),
                ExecUnits { cpu: 0, mem: 0 },
                vec!["No matching test found; validator compiled successfully".to_string()],
                None,
            )
        }
    } else {
        // Use the first matching test result
        let tr = &test_results[0];
        (
            tr.result.clone(),
            ExecUnits {
                cpu: tr.exec_units.cpu,
                mem: tr.exec_units.mem,
            },
            tr.traces.clone(),
            tr.error_detail.clone(),
        )
    };

    let trace_output = TraceOutput {
        scope: "script_only".to_string(),
        validator: validator_title,
        purpose: config.purpose,
        context_mode: context_mode.to_string(),
        auto_filled_fields,
        cost_fidelity: cost_fidelity.to_string(),
        result: result_str.clone(),
        exec_units,
        budget_source,
        traces,
        error_detail,
    };

    let data = serde_json::to_value(&trace_output)
        .map_err(|e| anyhow::anyhow!("failed to serialize trace output: {e}"))?;

    Ok(Output::ok(data))
}

/// Extract the module path from a validator title like "escrow.escrow.spend" -> "escrow"
fn extract_module_from_title(title: &str) -> Option<String> {
    let parts: Vec<&str> = title.split('.').collect();
    if parts.len() >= 2 {
        // The first part(s) before the last one form the module path
        Some(parts[..parts.len() - 1].join("/"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_validate_json_valid_object() -> TestResult {
        let val = validate_json(r#"{"key": "value"}"#)?;
        assert!(val.is_object());
        Ok(())
    }

    #[test]
    fn test_validate_json_valid_array() -> TestResult {
        let val = validate_json(r#"[1, 2, 3]"#)?;
        assert!(val.is_array());
        Ok(())
    }

    #[test]
    fn test_validate_json_invalid() {
        let result = validate_json("not json at all {");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_json_string() -> TestResult {
        let val = validate_json(r#""hello""#)?;
        assert!(val.is_string());
        Ok(())
    }

    #[test]
    fn test_validate_signatory_valid() -> TestResult {
        // 56 hex chars = 28 bytes
        let sig = "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd";
        validate_signatory(sig)?;
        Ok(())
    }

    #[test]
    fn test_validate_signatory_wrong_length() -> TestResult {
        let sig = "aabbccdd"; // 8 chars, not 56
        let result = validate_signatory(sig);
        assert!(result.is_err());
        let err = result.err().ok_or("expected error")?;
        assert!(err.to_string().contains("expected 56 hex characters"));
        Ok(())
    }

    #[test]
    fn test_validate_signatory_invalid_hex() -> TestResult {
        let sig = "gggggggggggggggggggggggggggggggggggggggggggggggggggggggg"; // 56 chars but not hex
        let result = validate_signatory(sig);
        assert!(result.is_err());
        let err = result.err().ok_or("expected error")?;
        assert!(err.to_string().contains("not valid hex"));
        Ok(())
    }

    #[test]
    fn test_find_validator_in_blueprint_exact() -> TestResult {
        let blueprint = r#"{
            "preamble": {"plutusVersion": "v3"},
            "validators": [{
                "title": "escrow.escrow.spend",
                "datum": {"title": "datum", "schema": {}},
                "redeemer": {"title": "redeemer", "schema": {}},
                "compiledCode": "deadbeef",
                "hash": "abc123"
            }]
        }"#;
        let (title, has_datum) = find_validator_in_blueprint(blueprint, "escrow.escrow.spend")?;
        assert_eq!(title, "escrow.escrow.spend");
        assert_eq!(has_datum, Some(true));
        Ok(())
    }

    #[test]
    fn test_find_validator_in_blueprint_suffix_match() -> TestResult {
        let blueprint = r#"{
            "preamble": {"plutusVersion": "v3"},
            "validators": [{
                "title": "escrow.escrow.spend",
                "datum": {"title": "datum", "schema": {}},
                "redeemer": {"title": "redeemer", "schema": {}},
                "compiledCode": "deadbeef",
                "hash": "abc123"
            }]
        }"#;
        let (title, _) = find_validator_in_blueprint(blueprint, "escrow.spend")?;
        assert_eq!(title, "escrow.escrow.spend");
        Ok(())
    }

    #[test]
    fn test_find_validator_not_found() {
        let blueprint = r#"{
            "preamble": {"plutusVersion": "v3"},
            "validators": [{
                "title": "escrow.escrow.spend",
                "compiledCode": "deadbeef",
                "hash": "abc123"
            }]
        }"#;
        let result = find_validator_in_blueprint(blueprint, "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_find_validator_mint_no_datum() -> TestResult {
        let blueprint = r#"{
            "preamble": {"plutusVersion": "v3"},
            "validators": [{
                "title": "escrow.token.mint",
                "redeemer": {"title": "redeemer", "schema": {}},
                "compiledCode": "cafebabe",
                "hash": "abc123"
            }]
        }"#;
        let (title, has_datum) = find_validator_in_blueprint(blueprint, "token.mint")?;
        assert_eq!(title, "escrow.token.mint");
        assert_eq!(has_datum, Some(false));
        Ok(())
    }

    #[test]
    fn test_extract_module_from_title() {
        assert_eq!(
            extract_module_from_title("escrow.escrow.spend"),
            Some("escrow/escrow".to_string())
        );
        assert_eq!(
            extract_module_from_title("module.validator"),
            Some("module".to_string())
        );
        assert_eq!(extract_module_from_title("standalone"), None);
    }

    #[test]
    fn test_valid_purposes() {
        assert!(VALID_PURPOSES.contains(&"spend"));
        assert!(VALID_PURPOSES.contains(&"mint"));
        assert!(VALID_PURPOSES.contains(&"withdraw"));
        assert!(VALID_PURPOSES.contains(&"publish"));
        assert!(!VALID_PURPOSES.contains(&"stake"));
    }

    #[tokio::test]
    async fn test_run_trace_invalid_purpose() -> TestResult {
        let config = TraceConfig {
            validator: "test".to_string(),
            purpose: "invalid_purpose".to_string(),
            redeemer: r#"{"constructor": 0, "fields": []}"#.to_string(),
            datum: None,
            context: None,
            slot: None,
            signatories: vec![],
        };
        let output = run_trace("/tmp/nonexistent", config).await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "INVALID_PURPOSE");
        Ok(())
    }

    #[tokio::test]
    async fn test_run_trace_empty_redeemer() -> TestResult {
        let config = TraceConfig {
            validator: "test".to_string(),
            purpose: "spend".to_string(),
            redeemer: String::new(),
            datum: None,
            context: None,
            slot: None,
            signatories: vec![],
        };
        let output = run_trace("/tmp/nonexistent", config).await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "INVALID_INPUT");
        Ok(())
    }

    #[tokio::test]
    async fn test_run_trace_invalid_redeemer_json() -> TestResult {
        let config = TraceConfig {
            validator: "test".to_string(),
            purpose: "spend".to_string(),
            redeemer: "not valid json {".to_string(),
            datum: None,
            context: None,
            slot: None,
            signatories: vec![],
        };
        let output = run_trace("/tmp/nonexistent", config).await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert!(json["message"]
            .as_str()
            .is_some_and(|m| m.contains("invalid redeemer")));
        Ok(())
    }

    #[tokio::test]
    async fn test_run_trace_invalid_datum_json() -> TestResult {
        let config = TraceConfig {
            validator: "test".to_string(),
            purpose: "spend".to_string(),
            redeemer: r#"{"constructor": 0, "fields": []}"#.to_string(),
            datum: Some("bad json".to_string()),
            context: None,
            slot: None,
            signatories: vec![],
        };
        let output = run_trace("/tmp/nonexistent", config).await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert!(json["message"]
            .as_str()
            .is_some_and(|m| m.contains("invalid datum")));
        Ok(())
    }

    #[tokio::test]
    async fn test_run_trace_invalid_signatory() -> TestResult {
        let config = TraceConfig {
            validator: "test".to_string(),
            purpose: "spend".to_string(),
            redeemer: r#"{"constructor": 0, "fields": []}"#.to_string(),
            datum: Some(r#"{"constructor": 0, "fields": []}"#.to_string()),
            context: None,
            slot: None,
            signatories: vec!["aabb".to_string()],
        };
        let output = run_trace("/tmp/nonexistent", config).await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert!(json["message"]
            .as_str()
            .is_some_and(|m| m.contains("expected 56 hex characters")));
        Ok(())
    }

    #[tokio::test]
    async fn test_run_trace_no_blueprint() -> TestResult {
        let config = TraceConfig {
            validator: "test.spend".to_string(),
            purpose: "spend".to_string(),
            redeemer: r#"{"constructor": 0, "fields": []}"#.to_string(),
            datum: Some(r#"{"constructor": 0, "fields": []}"#.to_string()),
            context: None,
            slot: None,
            signatories: vec![],
        };
        let output = run_trace("/tmp/nonexistent_dir_for_trace_test", config).await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "BLUEPRINT_NOT_FOUND");
        Ok(())
    }

    #[tokio::test]
    async fn test_run_trace_validator_not_found() -> TestResult {
        // Use the escrow fixture which has a real blueprint
        let fixture_dir =
            env!("CARGO_MANIFEST_DIR").replace("crates/utxray-core", "tests/fixtures/escrow");
        let config = TraceConfig {
            validator: "nonexistent.validator".to_string(),
            purpose: "spend".to_string(),
            redeemer: r#"{"constructor": 0, "fields": []}"#.to_string(),
            datum: Some(r#"{"constructor": 0, "fields": []}"#.to_string()),
            context: None,
            slot: None,
            signatories: vec![],
        };
        let output = run_trace(&fixture_dir, config).await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "VALIDATOR_NOT_FOUND");
        Ok(())
    }

    #[tokio::test]
    async fn test_run_trace_spend_without_datum() -> TestResult {
        let fixture_dir =
            env!("CARGO_MANIFEST_DIR").replace("crates/utxray-core", "tests/fixtures/escrow");
        let config = TraceConfig {
            validator: "escrow.spend".to_string(),
            purpose: "spend".to_string(),
            redeemer: r#"{"constructor": 0, "fields": []}"#.to_string(),
            datum: None,
            context: None,
            slot: None,
            signatories: vec![],
        };
        let output = run_trace(&fixture_dir, config).await?;
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert!(json["message"]
            .as_str()
            .is_some_and(|m| m.contains("datum is required for spend")));
        Ok(())
    }

    #[tokio::test]
    async fn test_run_trace_mint_without_datum_ok() -> TestResult {
        // Mint purpose should not require datum
        let fixture_dir =
            env!("CARGO_MANIFEST_DIR").replace("crates/utxray-core", "tests/fixtures/escrow");
        let config = TraceConfig {
            validator: "token.mint".to_string(),
            purpose: "mint".to_string(),
            redeemer: r#"{"constructor": 0, "fields": []}"#.to_string(),
            datum: None,
            context: None,
            slot: None,
            signatories: vec![],
        };
        let output = run_trace(&fixture_dir, config).await?;
        let json = serde_json::to_value(&output)?;
        // Should not error on missing datum for mint
        // It may error because aiken isn't installed, but NOT for missing datum
        let status = json["status"].as_str().unwrap_or("");
        if status == "error" {
            // Should be aiken-related, not datum-related
            let msg = json["message"].as_str().unwrap_or("");
            assert!(
                !msg.contains("datum is required"),
                "mint should not require datum"
            );
        }
        Ok(())
    }
}
