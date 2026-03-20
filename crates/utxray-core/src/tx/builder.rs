use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::output::Output;

/// Error types for tx build operations.
#[derive(Debug, thiserror::Error)]
pub enum TxBuildError {
    #[error("missing required field: {0}")]
    MissingField(String),
    #[error("invalid tx spec: {0}")]
    InvalidSpec(String),
    #[error("failed to read spec file: {0}")]
    ReadError(String),
    #[error("failed to write tx file: {0}")]
    WriteError(String),
}

/// A single input (pubkey-controlled UTxO).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TxInput {
    pub utxo: String,
    #[serde(rename = "type")]
    pub input_type: String,
}

/// A script input with datum, redeemer, etc.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScriptInput {
    pub utxo: String,
    pub validator: String,
    pub purpose: String,
    pub datum: serde_json::Value,
    pub redeemer: serde_json::Value,
    #[serde(default)]
    pub datum_source: Option<String>,
}

/// A transaction output.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TxOutput {
    pub address: String,
    pub value: TxValue,
    #[serde(default)]
    pub datum: Option<serde_json::Value>,
}

/// Value with lovelace and optional tokens.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TxValue {
    pub lovelace: u64,
    #[serde(default)]
    pub tokens: Option<HashMap<String, HashMap<String, u64>>>,
}

/// Mint entry for a policy.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MintEntry {
    pub assets: HashMap<String, u64>,
    pub redeemer: serde_json::Value,
    pub validator: String,
}

/// Validity interval.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Validity {
    pub from_slot: Option<u64>,
    pub to_slot: Option<u64>,
}

/// The full tx-spec.json structure.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TxSpec {
    pub inputs: Vec<TxInput>,
    #[serde(default)]
    pub script_inputs: Vec<ScriptInput>,
    #[serde(default)]
    pub reference_inputs: Vec<serde_json::Value>,
    pub outputs: Vec<TxOutput>,
    #[serde(default)]
    pub mint: Option<HashMap<String, MintEntry>>,
    #[serde(default)]
    pub collateral: Option<String>,
    pub change_address: String,
    #[serde(default)]
    pub required_signers: Vec<String>,
    #[serde(default)]
    pub validity: Option<Validity>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// A script invocation summary entry.
#[derive(Debug, Clone, Serialize)]
pub struct ScriptInvoked {
    pub name: String,
    pub purpose: String,
}

/// The summary block in the output.
#[derive(Debug, Clone, Serialize)]
pub struct TxBuildSummary {
    pub inputs_count: usize,
    pub outputs_count: usize,
    pub scripts_invoked: Vec<ScriptInvoked>,
    pub total_input_lovelace: u64,
    pub total_output_lovelace: u64,
    pub estimated_fee: u64,
}

/// Successful tx build output data.
#[derive(Debug, Serialize)]
pub struct TxBuildOutput {
    pub tx_file: String,
    pub summary: TxBuildSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_cbor: Option<String>,
}

/// Error output data for tx build.
#[derive(Debug, Serialize)]
pub struct TxBuildErrorOutput {
    pub error_code: String,
    pub message: String,
}

/// Parse and validate a tx spec from JSON string.
pub fn parse_tx_spec(json_str: &str) -> Result<TxSpec, TxBuildError> {
    serde_json::from_str::<TxSpec>(json_str).map_err(|e| TxBuildError::InvalidSpec(e.to_string()))
}

/// Validate that a parsed TxSpec has all required fields with reasonable values.
pub fn validate_tx_spec(spec: &TxSpec) -> Result<(), TxBuildError> {
    if spec.inputs.is_empty() && spec.script_inputs.is_empty() {
        return Err(TxBuildError::MissingField(
            "at least one input or script_input is required".to_string(),
        ));
    }
    if spec.outputs.is_empty() {
        return Err(TxBuildError::MissingField(
            "at least one output is required".to_string(),
        ));
    }
    if spec.change_address.is_empty() {
        return Err(TxBuildError::MissingField("change_address".to_string()));
    }

    // If there are script inputs, collateral should be present
    if !spec.script_inputs.is_empty() && spec.collateral.is_none() {
        return Err(TxBuildError::MissingField(
            "collateral is required when script_inputs are present".to_string(),
        ));
    }

    for (i, input) in spec.inputs.iter().enumerate() {
        if input.utxo.is_empty() {
            return Err(TxBuildError::InvalidSpec(format!(
                "input[{i}].utxo is empty"
            )));
        }
    }

    for (i, si) in spec.script_inputs.iter().enumerate() {
        if si.utxo.is_empty() {
            return Err(TxBuildError::InvalidSpec(format!(
                "script_inputs[{i}].utxo is empty"
            )));
        }
        if si.validator.is_empty() {
            return Err(TxBuildError::InvalidSpec(format!(
                "script_inputs[{i}].validator is empty"
            )));
        }
    }

    for (i, output) in spec.outputs.iter().enumerate() {
        if output.address.is_empty() {
            return Err(TxBuildError::InvalidSpec(format!(
                "outputs[{i}].address is empty"
            )));
        }
    }

    Ok(())
}

/// Extract script invocations from the spec.
pub fn extract_scripts_invoked(spec: &TxSpec) -> Vec<ScriptInvoked> {
    let mut scripts = Vec::new();

    for si in &spec.script_inputs {
        scripts.push(ScriptInvoked {
            name: si.validator.clone(),
            purpose: si.purpose.clone(),
        });
    }

    if let Some(ref mint_map) = spec.mint {
        for entry in mint_map.values() {
            scripts.push(ScriptInvoked {
                name: entry.validator.clone(),
                purpose: "mint".to_string(),
            });
        }
    }

    scripts
}

/// Calculate total output lovelace.
pub fn total_output_lovelace(spec: &TxSpec) -> u64 {
    spec.outputs.iter().map(|o| o.value.lovelace).sum()
}

/// Estimate fee (simplified for Phase 1 -- fixed estimate based on tx complexity).
pub fn estimate_fee(spec: &TxSpec) -> u64 {
    let base_fee: u64 = 170_000;
    let per_input: u64 = 5_000;
    let per_output: u64 = 5_000;
    let per_script: u64 = 10_000;

    let input_count = (spec.inputs.len() + spec.script_inputs.len()) as u64;
    let output_count = spec.outputs.len() as u64;
    let script_count =
        spec.script_inputs.len() as u64 + spec.mint.as_ref().map(|m| m.len() as u64).unwrap_or(0);

    base_fee + per_input * input_count + per_output * output_count + per_script * script_count
}

/// Build the transaction summary and write the tx file.
/// Returns Output with TxBuildOutput on success.
pub fn build_tx(
    spec: &TxSpec,
    tx_file_path: &str,
    include_raw: bool,
) -> Result<Output<serde_json::Value>, TxBuildError> {
    let scripts_invoked = extract_scripts_invoked(spec);
    let total_out = total_output_lovelace(spec);
    let fee = estimate_fee(spec);
    let inputs_count = spec.inputs.len() + spec.script_inputs.len();
    let outputs_count = spec.outputs.len();

    // For Phase 1, total_input_lovelace = total_output_lovelace + fee
    // (since we don't have real UTxO data to look up actual input values)
    let total_input_lovelace = total_out + fee;

    let summary = TxBuildSummary {
        inputs_count,
        outputs_count,
        scripts_invoked,
        total_input_lovelace,
        total_output_lovelace: total_out,
        estimated_fee: fee,
    };

    // Write the tx file (a JSON representation for Phase 1)
    let tx_content = serde_json::json!({
        "type": "utxray-unsigned-tx",
        "description": "Transaction built by utxray (Phase 1 - local only)",
        "spec": spec,
        "summary": {
            "inputs_count": summary.inputs_count,
            "outputs_count": summary.outputs_count,
            "estimated_fee": summary.estimated_fee,
        }
    });

    let tx_json = serde_json::to_string_pretty(&tx_content)
        .map_err(|e| TxBuildError::WriteError(format!("failed to serialize tx: {e}")))?;

    std::fs::write(tx_file_path, &tx_json)
        .map_err(|e| TxBuildError::WriteError(format!("failed to write {tx_file_path}: {e}")))?;

    let tx_cbor = if include_raw {
        // For Phase 1, provide hex-encoded JSON as placeholder
        Some(hex::encode(&tx_json))
    } else {
        None
    };

    let mut data = serde_json::json!({
        "tx_file": tx_file_path,
        "summary": summary,
    });

    if let Some(cbor) = tx_cbor {
        data.as_object_mut()
            .ok_or_else(|| TxBuildError::WriteError("internal serialization error".to_string()))?
            .insert("tx_cbor".to_string(), serde_json::Value::String(cbor));
    }

    Ok(Output::ok(data))
}

/// Top-level entry point: read spec file, validate, build.
pub fn run_tx_build(
    spec_path: &str,
    _exec_units_path: Option<&str>,
    tx_output_path: &str,
    include_raw: bool,
) -> Result<Output<serde_json::Value>, TxBuildError> {
    let spec_content = std::fs::read_to_string(spec_path)
        .map_err(|e| TxBuildError::ReadError(format!("{spec_path}: {e}")))?;

    let spec = parse_tx_spec(&spec_content)?;
    validate_tx_spec(&spec)?;
    build_tx(&spec, tx_output_path, include_raw)
}

/// Run tx build, returning Output (wrapping errors into error Output).
pub fn run_tx_build_safe(
    spec_path: &str,
    exec_units_path: Option<&str>,
    tx_output_path: &str,
    include_raw: bool,
) -> Output<serde_json::Value> {
    match run_tx_build(spec_path, exec_units_path, tx_output_path, include_raw) {
        Ok(output) => output,
        Err(e) => Output::error(serde_json::json!({
            "error_code": "TX_BUILD_FAILED",
            "message": e.to_string(),
        })),
    }
}

/// Resolve the tx output file path. If not specified, defaults to "./tx.unsigned"
/// relative to the spec file's parent directory.
pub fn resolve_tx_output_path(spec_path: &str) -> String {
    let spec = Path::new(spec_path);
    let parent = spec.parent().unwrap_or_else(|| Path::new("."));
    parent.join("tx.unsigned").to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn sample_spec_json() -> &'static str {
        r#"{
            "inputs": [{"utxo": "abc123#0", "type": "pubkey"}],
            "script_inputs": [
                {
                    "utxo": "def456#1",
                    "validator": "escrow.spend",
                    "purpose": "spend",
                    "datum": {"owner": "aabb", "deadline": 1000, "amount": 5000000},
                    "redeemer": {"constructor": 0, "fields": []},
                    "datum_source": "inline"
                }
            ],
            "reference_inputs": [],
            "outputs": [
                {"address": "addr_test1qz123", "value": {"lovelace": 5000000}},
                {"address": "addr_test1wq456", "value": {"lovelace": 2000000}}
            ],
            "mint": null,
            "collateral": "abc123#2",
            "change_address": "addr_test1qz123",
            "required_signers": ["aabb"],
            "validity": {"from_slot": null, "to_slot": 2000},
            "metadata": null
        }"#
    }

    #[test]
    fn test_parse_valid_spec() -> TestResult {
        let spec = parse_tx_spec(sample_spec_json())?;
        assert_eq!(spec.inputs.len(), 1);
        assert_eq!(spec.script_inputs.len(), 1);
        assert_eq!(spec.outputs.len(), 2);
        assert_eq!(spec.change_address, "addr_test1qz123");
        Ok(())
    }

    #[test]
    fn test_parse_invalid_json() {
        let result = parse_tx_spec("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_spec_ok() -> TestResult {
        let spec = parse_tx_spec(sample_spec_json())?;
        validate_tx_spec(&spec)?;
        Ok(())
    }

    #[test]
    fn test_validate_no_inputs() -> TestResult {
        let json = r#"{
            "inputs": [], "script_inputs": [],
            "outputs": [{"address": "addr", "value": {"lovelace": 1}}],
            "change_address": "addr"
        }"#;
        let spec = parse_tx_spec(json)?;
        let result = validate_tx_spec(&spec);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_validate_no_outputs() -> TestResult {
        let json = r#"{
            "inputs": [{"utxo": "abc#0", "type": "pubkey"}],
            "outputs": [],
            "change_address": "addr"
        }"#;
        let spec = parse_tx_spec(json)?;
        let result = validate_tx_spec(&spec);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_validate_missing_collateral() -> TestResult {
        let json = r#"{
            "inputs": [],
            "script_inputs": [
                {"utxo": "def#1", "validator": "v.spend", "purpose": "spend",
                 "datum": {}, "redeemer": {}}
            ],
            "outputs": [{"address": "addr", "value": {"lovelace": 1}}],
            "change_address": "addr"
        }"#;
        let spec = parse_tx_spec(json)?;
        let result = validate_tx_spec(&spec);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_extract_scripts_invoked() -> TestResult {
        let spec = parse_tx_spec(sample_spec_json())?;
        let scripts = extract_scripts_invoked(&spec);
        assert_eq!(scripts.len(), 1);
        assert_eq!(scripts[0].name, "escrow.spend");
        assert_eq!(scripts[0].purpose, "spend");
        Ok(())
    }

    #[test]
    fn test_extract_scripts_with_mint() -> TestResult {
        let json = r#"{
            "inputs": [{"utxo": "abc#0", "type": "pubkey"}],
            "script_inputs": [
                {"utxo": "def#1", "validator": "escrow.spend", "purpose": "spend",
                 "datum": {}, "redeemer": {}}
            ],
            "outputs": [{"address": "addr", "value": {"lovelace": 1}}],
            "mint": {
                "d4e5f6": {
                    "assets": {"MyToken": 1},
                    "redeemer": {"constructor": 0, "fields": []},
                    "validator": "token.mint"
                }
            },
            "collateral": "abc#2",
            "change_address": "addr"
        }"#;
        let spec = parse_tx_spec(json)?;
        let scripts = extract_scripts_invoked(&spec);
        assert_eq!(scripts.len(), 2);
        let names: Vec<&str> = scripts.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"escrow.spend"));
        assert!(names.contains(&"token.mint"));
        Ok(())
    }

    #[test]
    fn test_total_output_lovelace() -> TestResult {
        let spec = parse_tx_spec(sample_spec_json())?;
        assert_eq!(total_output_lovelace(&spec), 7_000_000);
        Ok(())
    }

    #[test]
    fn test_estimate_fee() -> TestResult {
        let spec = parse_tx_spec(sample_spec_json())?;
        let fee = estimate_fee(&spec);
        // base 170_000 + 2 inputs * 5_000 + 2 outputs * 5_000 + 1 script * 10_000
        assert_eq!(fee, 170_000 + 10_000 + 10_000 + 10_000);
        Ok(())
    }

    #[test]
    fn test_build_tx_writes_file() -> TestResult {
        let spec = parse_tx_spec(sample_spec_json())?;
        let dir = std::env::temp_dir().join("utxray_test_tx_build");
        std::fs::create_dir_all(&dir)?;
        let tx_path = dir.join("tx.unsigned");
        let tx_path_str = tx_path.to_string_lossy().to_string();

        let output = build_tx(&spec, &tx_path_str, false)?;
        let json = serde_json::to_value(&output)?;

        assert_eq!(json["status"], "ok");
        assert!(json["tx_file"].is_string());
        assert!(json["summary"]["inputs_count"].is_number());
        assert!(json.get("tx_cbor").is_none());
        assert!(tx_path.exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn test_build_tx_with_include_raw() -> TestResult {
        let spec = parse_tx_spec(sample_spec_json())?;
        let dir = std::env::temp_dir().join("utxray_test_tx_build_raw");
        std::fs::create_dir_all(&dir)?;
        let tx_path = dir.join("tx.unsigned");
        let tx_path_str = tx_path.to_string_lossy().to_string();

        let output = build_tx(&spec, &tx_path_str, true)?;
        let json = serde_json::to_value(&output)?;

        assert_eq!(json["status"], "ok");
        assert!(json["tx_cbor"].is_string());

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn test_resolve_tx_output_path() {
        let path = resolve_tx_output_path("/some/dir/spec.json");
        assert!(path.ends_with("tx.unsigned"));
        assert!(path.contains("/some/dir/"));
    }

    #[test]
    fn test_run_tx_build_safe_file_not_found() -> TestResult {
        let output = run_tx_build_safe("/nonexistent/spec.json", None, "/tmp/tx.unsigned", false);
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["status"], "error");
        assert_eq!(json["error_code"], "TX_BUILD_FAILED");
        Ok(())
    }
}
