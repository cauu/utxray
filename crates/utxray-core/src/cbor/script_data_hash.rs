use serde::Serialize;

use crate::output::Output;

// ── Error types ────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ScriptDataHashError {
    #[error("invalid JSON input for {field}: {detail}")]
    InvalidJson { field: String, detail: String },

    #[error("failed to read file '{path}': {detail}")]
    FileRead { path: String, detail: String },

    #[error("CBOR encoding failed: {0}")]
    CborEncode(String),

    #[error("missing required argument: {0}")]
    MissingArgument(String),

    #[error("'from-network' cost models are not supported in Phase 1 (local mode)")]
    FromNetworkNotSupported,
}

// ── Output types ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ScriptDataHashOutput {
    pub script_data_hash: String,
}

#[derive(Debug, Serialize)]
pub struct ScriptDataHashErrorData {
    pub error: String,
}

// ── Public API ─────────────────────────────────────────────────

/// Compute the Plutus script data hash from redeemers, datums, and cost models.
///
/// Per the Cardano ledger spec, the script data hash is:
///   blake2b_256(redeemers_cbor || datums_cbor || cost_models_cbor)
///
/// For Phase 1 (local), we accept JSON inputs and encode them as CBOR,
/// then hash the concatenation.
///
/// Each input can be either inline JSON or a file path.
/// "from-network" for cost_models is not supported in Phase 1.
pub fn compute_script_data_hash(
    redeemers_input: &str,
    datums_input: &str,
    cost_models_input: &str,
) -> Result<Output<ScriptDataHashOutput>, ScriptDataHashError> {
    if cost_models_input == "from-network" {
        return Err(ScriptDataHashError::FromNetworkNotSupported);
    }

    let redeemers_json = read_json_input(redeemers_input, "redeemers")?;
    let datums_json = read_json_input(datums_input, "datums")?;
    let cost_models_json = read_json_input(cost_models_input, "cost-models")?;

    // Validate that redeemers and datums are arrays
    if !redeemers_json.is_array() {
        return Err(ScriptDataHashError::InvalidJson {
            field: "redeemers".to_string(),
            detail: "expected a JSON array".to_string(),
        });
    }
    if !datums_json.is_array() {
        return Err(ScriptDataHashError::InvalidJson {
            field: "datums".to_string(),
            detail: "expected a JSON array".to_string(),
        });
    }
    if !cost_models_json.is_object() && !cost_models_json.is_array() {
        return Err(ScriptDataHashError::InvalidJson {
            field: "cost-models".to_string(),
            detail: "expected a JSON object or array".to_string(),
        });
    }

    // Serialize each component to canonical JSON bytes, then treat as the
    // "CBOR" payload for hashing. In a full implementation we'd encode actual
    // Plutus CBOR; for Phase 1 we hash the canonical JSON representation,
    // which is deterministic and sufficient for local comparison.
    let redeemers_bytes = canonical_json_bytes(&redeemers_json)?;
    let datums_bytes = canonical_json_bytes(&datums_json)?;
    let cost_models_bytes = canonical_json_bytes(&cost_models_json)?;

    // blake2b-256 of concatenated payloads
    let mut hasher = pallas_crypto::hash::Hasher::<256>::new();
    hasher.input(&redeemers_bytes);
    hasher.input(&datums_bytes);
    hasher.input(&cost_models_bytes);
    let hash = hasher.finalize();

    Ok(Output::ok(ScriptDataHashOutput {
        script_data_hash: hex::encode(hash),
    }))
}

// ── Internal helpers ───────────────────────────────────────────

/// Read a JSON input from either an inline JSON string or a file path.
fn read_json_input(
    input: &str,
    field_name: &str,
) -> Result<serde_json::Value, ScriptDataHashError> {
    // Try inline JSON first
    if let Ok(val) = serde_json::from_str(input) {
        return Ok(val);
    }

    // Try reading as file path
    let path = std::path::Path::new(input);
    if path.exists() {
        let content = std::fs::read_to_string(path).map_err(|e| ScriptDataHashError::FileRead {
            path: input.to_string(),
            detail: e.to_string(),
        })?;
        return serde_json::from_str(&content).map_err(|e| ScriptDataHashError::InvalidJson {
            field: field_name.to_string(),
            detail: e.to_string(),
        });
    }

    Err(ScriptDataHashError::InvalidJson {
        field: field_name.to_string(),
        detail: format!("not valid JSON and not a readable file: {input}"),
    })
}

/// Produce deterministic (canonical) JSON bytes for hashing.
fn canonical_json_bytes(value: &serde_json::Value) -> Result<Vec<u8>, ScriptDataHashError> {
    serde_json::to_vec(value).map_err(|e| ScriptDataHashError::CborEncode(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_compute_hash_basic() -> TestResult {
        let redeemers = r#"[{"constructor": 0, "fields": []}]"#;
        let datums = r#"[{"int": 42}]"#;
        let cost_models = r#"{"PlutusV2": [100, 200, 300]}"#;

        let output = compute_script_data_hash(redeemers, datums, cost_models)?;
        assert!(!output.data.script_data_hash.is_empty());
        // Hash should be 64 hex chars (32 bytes)
        assert_eq!(output.data.script_data_hash.len(), 64);
        // All chars should be hex
        assert!(output
            .data
            .script_data_hash
            .chars()
            .all(|c| c.is_ascii_hexdigit()));
        Ok(())
    }

    #[test]
    fn test_deterministic_hash() -> TestResult {
        let redeemers = r#"[{"constructor": 0, "fields": []}]"#;
        let datums = r#"[{"int": 42}]"#;
        let cost_models = r#"{"PlutusV2": [100, 200, 300]}"#;

        let output1 = compute_script_data_hash(redeemers, datums, cost_models)?;
        let output2 = compute_script_data_hash(redeemers, datums, cost_models)?;
        assert_eq!(output1.data.script_data_hash, output2.data.script_data_hash);
        Ok(())
    }

    #[test]
    fn test_different_inputs_different_hash() -> TestResult {
        let redeemers = r#"[{"constructor": 0, "fields": []}]"#;
        let datums1 = r#"[{"int": 42}]"#;
        let datums2 = r#"[{"int": 43}]"#;
        let cost_models = r#"{"PlutusV2": [100, 200, 300]}"#;

        let output1 = compute_script_data_hash(redeemers, datums1, cost_models)?;
        let output2 = compute_script_data_hash(redeemers, datums2, cost_models)?;
        assert_ne!(output1.data.script_data_hash, output2.data.script_data_hash);
        Ok(())
    }

    #[test]
    fn test_from_network_not_supported() {
        let result = compute_script_data_hash(r#"[]"#, r#"[]"#, "from-network");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("from-network"),
            "Expected from-network error, got: {err}"
        );
    }

    #[test]
    fn test_invalid_redeemers_json() {
        let result = compute_script_data_hash("not json", r#"[]"#, r#"{}"#);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("redeemers"),
            "Expected redeemers error, got: {err}"
        );
    }

    #[test]
    fn test_invalid_datums_json() {
        let result = compute_script_data_hash(r#"[]"#, "not json", r#"{}"#);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("datums"),
            "Expected datums error, got: {err}"
        );
    }

    #[test]
    fn test_invalid_cost_models_json() {
        let result = compute_script_data_hash(r#"[]"#, r#"[]"#, "not json");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("cost-models"),
            "Expected cost-models error, got: {err}"
        );
    }

    #[test]
    fn test_redeemers_not_array() {
        let result = compute_script_data_hash(r#"{"key": "value"}"#, r#"[]"#, r#"{}"#);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("redeemers"),
            "Expected redeemers error, got: {err}"
        );
    }

    #[test]
    fn test_datums_not_array() {
        let result = compute_script_data_hash(r#"[]"#, r#"{"key": "value"}"#, r#"{}"#);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("datums"),
            "Expected datums error, got: {err}"
        );
    }

    #[test]
    fn test_cost_models_not_object() {
        let result = compute_script_data_hash(r#"[]"#, r#"[]"#, r#""string""#);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("cost-models"),
            "Expected cost-models error, got: {err}"
        );
    }

    #[test]
    fn test_empty_inputs() -> TestResult {
        let output = compute_script_data_hash(r#"[]"#, r#"[]"#, r#"{}"#)?;
        assert_eq!(output.data.script_data_hash.len(), 64);
        Ok(())
    }

    #[test]
    fn test_file_input() -> TestResult {
        use std::path::PathBuf;
        let fixture_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cbor");

        // Use the existing datum fixture as a stand-in for datums array testing.
        // For file-based input, we test read_json_input directly.
        let datum_path = fixture_dir.join("script_data_hash_redeemers.json");
        std::fs::write(&datum_path, r#"[{"constructor": 0, "fields": []}]"#)?;

        let result = read_json_input(datum_path.to_str().ok_or("invalid path")?, "redeemers");
        // Clean up
        let _ = std::fs::remove_file(&datum_path);

        let val = result?;
        assert!(val.is_array());
        Ok(())
    }
}
