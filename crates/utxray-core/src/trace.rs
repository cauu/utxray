use std::path::Path;

use serde::Serialize;

use crate::aiken::cli::AikenCli;
use crate::build::parse_blueprint;
use crate::error::BudgetSource;
use crate::output::Output;
use crate::test_cmd::{parse_test_output, ExecUnits};

/// Valid purposes for the trace command (spec names + common aliases).
const VALID_PURPOSES: &[&str] = &[
    "spend",
    "mint",
    "withdrawal",
    "withdraw",
    "certificate",
    "cert",
    "propose",
    "vote",
];

/// Normalize a purpose alias to the canonical spec name.
/// `withdraw` -> `withdrawal`, `cert` -> `certificate`. Others unchanged.
fn normalize_purpose(purpose: &str) -> &str {
    match purpose {
        "withdraw" => "withdrawal",
        "cert" => "certificate",
        _ => purpose,
    }
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constructed_context: Option<serde_json::Value>,
    pub execution_performed: bool,
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

    #[error("purpose '{0}' is invalid; expected one of: spend, mint, withdrawal, certificate, propose, vote (aliases: withdraw, cert)")]
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

/// Information about a validator found in the blueprint.
struct BlueprintValidatorInfo {
    title: String,
    has_datum: Option<bool>,
    hash: String,
}

/// Look up a validator in the blueprint by name. Supports both "module.name" and just "name".
fn find_validator_in_blueprint(
    blueprint_content: &str,
    validator_name: &str,
) -> Result<BlueprintValidatorInfo, TraceError> {
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
            let hash = v
                .get("hash")
                .and_then(|h| h.as_str())
                .unwrap_or("")
                .to_string();
            return Ok(BlueprintValidatorInfo {
                title: title.to_string(),
                has_datum: Some(has_datum),
                hash,
            });
        }
    }

    // Try matching against parsed validators by name
    for v in &validators {
        if v.name == validator_name {
            return Ok(BlueprintValidatorInfo {
                title: validator_name.to_string(),
                has_datum: None,
                hash: v.hash.clone(),
            });
        }
    }

    Err(TraceError::ValidatorNotFound(validator_name.to_string()))
}

/// Build a minimal ScriptContext JSON for Mode A (no user-supplied context).
///
/// Constructs a well-formed Aiken-compatible ScriptContext from the provided
/// inputs, following the auto-fill algorithm from the spec.
fn build_minimal_script_context(
    purpose: &str,
    validator_hash: &str,
    datum: Option<&serde_json::Value>,
    redeemer: &serde_json::Value,
    slot: Option<u64>,
    signatories: &[String],
) -> serde_json::Value {
    // Zero tx hash for the fake script input
    let zero_hash = "0000000000000000000000000000000000000000000000000000000000000000";
    // Distinct hash for the fee-providing pubkey input
    let fee_hash = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

    // Build the script input based on purpose
    let script_credential = serde_json::json!({
        "Script": validator_hash
    });

    let script_address = serde_json::json!({
        "payment_credential": script_credential,
        "stake_credential": null
    });

    // The script UTxO input (for spend) or a reference input
    let script_input_value = serde_json::json!({
        "lovelace": 5_000_000u64,
        "assets": {}
    });

    let mut script_input = serde_json::json!({
        "output_reference": {
            "transaction_id": zero_hash,
            "output_index": 0
        },
        "output": {
            "address": script_address,
            "value": script_input_value,
            "datum": null,
            "reference_script": null
        }
    });

    // For spend, attach inline datum to the script input
    if purpose == "spend" {
        if let Some(d) = datum {
            script_input["output"]["datum"] = serde_json::json!({
                "InlineDatum": d
            });
        }
    }

    // Fee-providing pubkey input (1 billion lovelace)
    let pubkey_hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let pubkey_address = serde_json::json!({
        "payment_credential": { "VerificationKey": &pubkey_hash[..56] },
        "stake_credential": null
    });

    let fee_input = serde_json::json!({
        "output_reference": {
            "transaction_id": fee_hash,
            "output_index": 0
        },
        "output": {
            "address": pubkey_address,
            "value": {
                "lovelace": 1_000_000_000u64,
                "assets": {}
            },
            "datum": null,
            "reference_script": null
        }
    });

    // Build inputs list: for spend, include the script input
    let inputs = if purpose == "spend" {
        serde_json::json!([script_input, fee_input])
    } else {
        serde_json::json!([fee_input])
    };

    // Build mint field: for mint purpose, include the validator's policy
    let mint = if purpose == "mint" {
        serde_json::json!({
            validator_hash: {
                "": 1
            }
        })
    } else {
        serde_json::json!({})
    };

    // Build withdrawals: for withdrawal purpose
    let withdrawals = if purpose == "withdrawal" {
        serde_json::json!({
            validator_hash: 0
        })
    } else {
        serde_json::json!({})
    };

    // Validity range
    let validity_from = slot.map(|s| serde_json::json!(s));
    let validity_to: Option<serde_json::Value> = None;
    let validity_range = serde_json::json!({
        "from": validity_from,
        "to": validity_to
    });

    // Redeemers map: construct based on purpose
    let redeemer_key = match purpose {
        "spend" => serde_json::json!({
            "Spend": {
                "transaction_id": zero_hash,
                "output_index": 0
            }
        }),
        "mint" => serde_json::json!({
            "Mint": validator_hash
        }),
        "withdrawal" => serde_json::json!({
            "Withdraw": validator_hash
        }),
        "certificate" => serde_json::json!({
            "Publish": 0
        }),
        "propose" => serde_json::json!({
            "Propose": 0
        }),
        "vote" => serde_json::json!({
            "Vote": validator_hash
        }),
        _ => serde_json::json!(null),
    };

    let redeemers = serde_json::json!({
        "entries": [{
            "key": redeemer_key,
            "value": redeemer
        }]
    });

    // Datums map
    let datums = if let Some(d) = datum {
        serde_json::json!({
            "entries": [{
                "key": "inline",
                "value": d
            }]
        })
    } else {
        serde_json::json!({ "entries": [] })
    };

    // Transaction ID (hash of zeros for minimal context)
    let tx_id = zero_hash;

    // Build the full transaction
    let transaction = serde_json::json!({
        "inputs": inputs,
        "reference_inputs": [],
        "outputs": [],
        "fee": 200_000u64,
        "mint": mint,
        "certificates": [],
        "withdrawals": withdrawals,
        "validity_range": validity_range,
        "signatories": signatories,
        "redeemers": redeemers,
        "datums": datums,
        "id": tx_id
    });

    // Build purpose-specific info
    let purpose_info = match purpose {
        "spend" => serde_json::json!({
            "Spend": {
                "output_reference": {
                    "transaction_id": zero_hash,
                    "output_index": 0
                },
                "datum": datum
            }
        }),
        "mint" => serde_json::json!({
            "Mint": validator_hash
        }),
        "withdrawal" => serde_json::json!({
            "Withdraw": {
                "Script": validator_hash
            }
        }),
        "certificate" => serde_json::json!({
            "Publish": {
                "index": 0,
                "certificate": null
            }
        }),
        "propose" => serde_json::json!({
            "Propose": {
                "index": 0,
                "proposal_procedure": null
            }
        }),
        "vote" => serde_json::json!({
            "Vote": {
                "voter": validator_hash
            }
        }),
        _ => serde_json::json!(null),
    };

    serde_json::json!({
        "transaction": transaction,
        "purpose": purpose_info,
        "redeemer": redeemer
    })
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

    // Normalize purpose alias
    let normalized_purpose = normalize_purpose(&config.purpose).to_string();

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

    let bp_info = match find_validator_in_blueprint(&blueprint_content, &config.validator) {
        Ok(v) => v,
        Err(e) => {
            let output = Output::error(serde_json::json!({
                "error_code": "VALIDATOR_NOT_FOUND",
                "message": e.to_string()
            }));
            return Ok(output);
        }
    };

    let validator_title = bp_info.title;
    let has_datum_schema = bp_info.has_datum;
    let validator_hash = bp_info.hash;

    // For spend purpose, datum is required if the blueprint declares a datum schema
    if normalized_purpose == "spend" && config.datum.is_none() {
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

    // Parse redeemer and datum JSON for context construction
    let redeemer_value = validate_json(&config.redeemer)
        .map_err(|e| anyhow::anyhow!("redeemer parse failed unexpectedly: {e}"))?;
    let datum_value = match &config.datum {
        Some(d) => Some(
            validate_json(d)
                .map_err(|e| anyhow::anyhow!("datum parse failed unexpectedly: {e}"))?,
        ),
        None => None,
    };

    // Build the minimal ScriptContext for Mode A (no user-supplied context)
    let constructed_context = if !has_context {
        Some(build_minimal_script_context(
            &normalized_purpose,
            &validator_hash,
            datum_value.as_ref(),
            &redeemer_value,
            config.slot,
            &config.signatories,
        ))
    } else {
        None
    };

    // Attempt to run via aiken
    let cli = match AikenCli::new(project_dir) {
        Ok(c) => c,
        Err(_) => {
            // Aiken not available — return ok with constructed context but no execution
            let trace_output = TraceOutput {
                scope: "script_only".to_string(),
                validator: validator_title,
                purpose: normalized_purpose.clone(),
                context_mode: context_mode.to_string(),
                auto_filled_fields,
                cost_fidelity: cost_fidelity.to_string(),
                result: "pending".to_string(),
                exec_units: ExecUnits { cpu: 0, mem: 0 },
                budget_source,
                traces: vec!["aiken not available; execution not performed".to_string()],
                error_detail: Some(
                    "trace execution requires aiken. Install it from https://aiken-lang.org"
                        .to_string(),
                ),
                constructed_context,
                execution_performed: false,
            };

            let data = serde_json::to_value(&trace_output)
                .map_err(|e| anyhow::anyhow!("failed to serialize trace output: {e}"))?;

            let output = Output::ok(data).with_warning(
                crate::error::Severity::Warning,
                "aiken not available; context constructed but execution was not performed",
            );
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
    let (result_str, exec_units, traces, error_detail, actually_executed) =
        if test_results.is_empty() {
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
                    false, // compilation failed, no execution
                )
            } else {
                (
                    "pass".to_string(),
                    ExecUnits { cpu: 0, mem: 0 },
                    vec!["No matching test found; validator compiled successfully".to_string()],
                    None,
                    false, // compiled but no test was executed
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
                true, // test actually ran
            )
        };

    let trace_output = TraceOutput {
        scope: "script_only".to_string(),
        validator: validator_title,
        purpose: normalized_purpose,
        context_mode: context_mode.to_string(),
        auto_filled_fields,
        cost_fidelity: cost_fidelity.to_string(),
        result: result_str.clone(),
        exec_units,
        budget_source,
        traces,
        error_detail,
        constructed_context,
        execution_performed: actually_executed,
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
        let info = find_validator_in_blueprint(blueprint, "escrow.escrow.spend")?;
        assert_eq!(info.title, "escrow.escrow.spend");
        assert_eq!(info.has_datum, Some(true));
        assert_eq!(info.hash, "abc123");
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
        let info = find_validator_in_blueprint(blueprint, "escrow.spend")?;
        assert_eq!(info.title, "escrow.escrow.spend");
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
        let info = find_validator_in_blueprint(blueprint, "token.mint")?;
        assert_eq!(info.title, "escrow.token.mint");
        assert_eq!(info.has_datum, Some(false));
        assert_eq!(info.hash, "abc123");
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
        assert!(VALID_PURPOSES.contains(&"withdrawal"));
        assert!(VALID_PURPOSES.contains(&"withdraw"));
        assert!(VALID_PURPOSES.contains(&"certificate"));
        assert!(VALID_PURPOSES.contains(&"cert"));
        assert!(VALID_PURPOSES.contains(&"propose"));
        assert!(VALID_PURPOSES.contains(&"vote"));
        assert!(!VALID_PURPOSES.contains(&"stake"));
        assert!(!VALID_PURPOSES.contains(&"publish"));
    }

    #[test]
    fn test_normalize_purpose() {
        assert_eq!(normalize_purpose("withdraw"), "withdrawal");
        assert_eq!(normalize_purpose("cert"), "certificate");
        assert_eq!(normalize_purpose("spend"), "spend");
        assert_eq!(normalize_purpose("mint"), "mint");
        assert_eq!(normalize_purpose("withdrawal"), "withdrawal");
        assert_eq!(normalize_purpose("certificate"), "certificate");
        assert_eq!(normalize_purpose("propose"), "propose");
        assert_eq!(normalize_purpose("vote"), "vote");
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
        let status = json["status"].as_str().unwrap_or("");
        // When aiken is not installed, we now get "ok" with a warning and constructed_context
        if status == "ok" {
            // Check that context was constructed and no datum error
            assert!(
                json.get("error_detail").is_none()
                    || !json["error_detail"]
                        .as_str()
                        .unwrap_or("")
                        .contains("datum is required"),
                "mint should not require datum"
            );
        } else if status == "error" {
            let msg = json["message"].as_str().unwrap_or("");
            assert!(
                !msg.contains("datum is required"),
                "mint should not require datum"
            );
        }
        Ok(())
    }

    #[test]
    fn test_build_minimal_context_spend() -> TestResult {
        let datum = serde_json::json!({"constructor": 0, "fields": [{"bytes": "aabb"}, {"int": 100}, {"int": 50}]});
        let redeemer = serde_json::json!({"constructor": 0, "fields": []});
        let hash = "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd";
        let signatories =
            vec!["aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd".to_string()];

        let ctx = build_minimal_script_context(
            "spend",
            hash,
            Some(&datum),
            &redeemer,
            Some(1000),
            &signatories,
        );

        // Verify top-level structure
        assert!(ctx.get("transaction").is_some());
        assert!(ctx.get("purpose").is_some());
        assert!(ctx.get("redeemer").is_some());

        let tx = &ctx["transaction"];
        // Verify transaction fields
        assert_eq!(tx["fee"], 200_000);
        assert!(tx["inputs"].as_array().is_some());
        assert_eq!(tx["inputs"].as_array().ok_or("expected array")?.len(), 2); // script + fee input
        assert!(tx["reference_inputs"]
            .as_array()
            .ok_or("expected array")?
            .is_empty());
        assert!(tx["outputs"].as_array().ok_or("expected array")?.is_empty());
        assert!(tx["certificates"]
            .as_array()
            .ok_or("expected array")?
            .is_empty());
        assert_eq!(
            tx["signatories"].as_array().ok_or("expected array")?.len(),
            1
        );

        // Verify validity range has the slot
        assert_eq!(tx["validity_range"]["from"], 1000);
        assert!(tx["validity_range"]["to"].is_null());

        // Verify the script input has inline datum
        let script_input = &tx["inputs"][0];
        assert!(script_input["output"]["datum"]["InlineDatum"].is_object());

        // Verify purpose is Spend
        assert!(ctx["purpose"].get("Spend").is_some());

        // Verify redeemers entry
        let redeemer_entries = tx["redeemers"]["entries"]
            .as_array()
            .ok_or("expected array")?;
        assert_eq!(redeemer_entries.len(), 1);
        assert!(redeemer_entries[0]["key"].get("Spend").is_some());
        Ok(())
    }

    #[test]
    fn test_build_minimal_context_mint() -> TestResult {
        let redeemer = serde_json::json!({"constructor": 0, "fields": []});
        let hash = "11223344112233441122334411223344112233441122334411223344";

        let ctx = build_minimal_script_context("mint", hash, None, &redeemer, None, &[]);

        let tx = &ctx["transaction"];
        // Mint should only have the fee input (no script input)
        assert_eq!(tx["inputs"].as_array().ok_or("expected array")?.len(), 1);

        // Mint field should contain the policy
        assert!(tx["mint"].get(hash).is_some());

        // Purpose should be Mint
        assert!(ctx["purpose"].get("Mint").is_some());
        assert_eq!(ctx["purpose"]["Mint"], hash);

        // Validity range should have null from (no slot provided)
        assert!(tx["validity_range"]["from"].is_null());

        // Redeemer key should be Mint
        let redeemer_entries = tx["redeemers"]["entries"]
            .as_array()
            .ok_or("expected array")?;
        assert!(redeemer_entries[0]["key"].get("Mint").is_some());
        Ok(())
    }

    #[test]
    fn test_build_minimal_context_withdraw() -> TestResult {
        let redeemer = serde_json::json!({"constructor": 1, "fields": []});
        let hash = "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd";

        let ctx = build_minimal_script_context("withdrawal", hash, None, &redeemer, None, &[]);

        let tx = &ctx["transaction"];
        // Withdrawals should contain the credential
        assert!(tx["withdrawals"].get(hash).is_some());

        // Purpose should be Withdraw
        assert!(ctx["purpose"].get("Withdraw").is_some());

        // Redeemer key should be Withdraw
        let redeemer_entries = tx["redeemers"]["entries"]
            .as_array()
            .ok_or("expected array")?;
        assert!(redeemer_entries[0]["key"].get("Withdraw").is_some());
        Ok(())
    }

    #[test]
    fn test_build_minimal_context_no_datum_for_non_spend() -> TestResult {
        let redeemer = serde_json::json!({"constructor": 0, "fields": []});
        let hash = "11223344112233441122334411223344112233441122334411223344";

        let ctx = build_minimal_script_context("mint", hash, None, &redeemer, None, &[]);

        // Datums entries should be empty
        let datums = &ctx["transaction"]["datums"]["entries"];
        assert!(datums.as_array().ok_or("expected array")?.is_empty());
        Ok(())
    }

    #[test]
    fn test_build_minimal_context_with_signatories() -> TestResult {
        let redeemer = serde_json::json!({"constructor": 0, "fields": []});
        let hash = "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd";
        let sigs = vec![
            "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd".to_string(),
            "11223344112233441122334411223344112233441122334411223344".to_string(),
        ];

        let ctx = build_minimal_script_context("spend", hash, None, &redeemer, None, &sigs);

        assert_eq!(
            ctx["transaction"]["signatories"]
                .as_array()
                .ok_or("expected array")?
                .len(),
            2
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_run_trace_constructs_context_mode_a() -> TestResult {
        let fixture_dir =
            env!("CARGO_MANIFEST_DIR").replace("crates/utxray-core", "tests/fixtures/escrow");
        let config = TraceConfig {
            validator: "escrow.spend".to_string(),
            purpose: "spend".to_string(),
            redeemer: r#"{"constructor": 0, "fields": []}"#.to_string(),
            datum: Some(
                r#"{"constructor": 0, "fields": [{"bytes": "aabb"}, {"int": 100}, {"int": 50}]}"#
                    .to_string(),
            ),
            context: None,
            slot: Some(500),
            signatories: vec![],
        };
        let output = run_trace(&fixture_dir, config).await?;
        let json = serde_json::to_value(&output)?;

        let status = json["status"].as_str().unwrap_or("");
        assert!(
            status == "ok" || status == "error",
            "status should be ok or error, got {status}"
        );

        // In Mode A (no user-supplied context), constructed_context should be present
        // regardless of whether aiken is available
        if status == "ok" {
            // constructed_context should be present
            assert!(
                json.get("constructed_context").is_some() && !json["constructed_context"].is_null(),
                "constructed_context should be present in Mode A"
            );

            // Verify the context has proper structure
            let ctx = &json["constructed_context"];
            assert!(ctx.get("transaction").is_some());
            assert!(ctx.get("purpose").is_some());
            assert!(ctx.get("redeemer").is_some());

            // Verify validity range has the slot we passed
            assert_eq!(ctx["transaction"]["validity_range"]["from"], 500);
        }
        Ok(())
    }
}
