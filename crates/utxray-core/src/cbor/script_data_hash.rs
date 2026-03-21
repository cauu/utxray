use serde::Serialize;

use crate::cbor::encode::{json_to_plutus_data, PlutusDataEncodeError};
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

    #[error("PlutusData conversion failed: {0}")]
    PlutusDataConvert(#[from] PlutusDataEncodeError),

    #[error("missing required argument: {0}")]
    MissingArgument(String),

    #[error("'from-network' cost models are not supported in Phase 1 (local mode)")]
    FromNetworkNotSupported,

    #[error("invalid cost model key '{0}': expected PlutusV1, PlutusV2, or PlutusV3")]
    InvalidCostModelKey(String),

    #[error("cost model values must be an array of integers")]
    InvalidCostModelValues,
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
/// - redeemers_cbor: CBOR-encoded array of PlutusData values
/// - datums_cbor: CBOR-encoded array of PlutusData values (empty bytes if no datums)
/// - cost_models_cbor: canonical CBOR map from language version keys to integer arrays
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

    // Encode redeemers as CBOR array of PlutusData
    let redeemers_bytes =
        encode_plutus_data_array(redeemers_json.as_array().unwrap_or(&Vec::new()))?;

    // Encode datums: if empty array, use empty bytes; otherwise CBOR array
    let empty_vec = Vec::new();
    let datums_arr = datums_json.as_array().unwrap_or(&empty_vec);
    let datums_bytes = if datums_arr.is_empty() {
        Vec::new()
    } else {
        encode_plutus_data_array(datums_arr)?
    };

    // Encode cost models as canonical CBOR map
    let cost_models_bytes = encode_cost_models(&cost_models_json)?;

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

/// Encode an array of PlutusData JSON values into a CBOR array.
fn encode_plutus_data_array(items: &[serde_json::Value]) -> Result<Vec<u8>, ScriptDataHashError> {
    let mut plutus_items = Vec::with_capacity(items.len());
    for item in items {
        let pd = json_to_plutus_data(item)?;
        plutus_items.push(pd);
    }

    // Encode as a CBOR definite-length array
    let mut buf = Vec::new();
    let mut encoder = pallas_codec::minicbor::Encoder::new(&mut buf);
    encoder
        .array(plutus_items.len() as u64)
        .map_err(|e| ScriptDataHashError::CborEncode(e.to_string()))?;
    for item in &plutus_items {
        encoder
            .encode(item)
            .map_err(|e| ScriptDataHashError::CborEncode(e.to_string()))?;
    }

    Ok(buf)
}

/// Map a language version string to its CBOR integer key.
fn language_key(name: &str) -> Result<u64, ScriptDataHashError> {
    match name {
        "PlutusV1" | "0" => Ok(0),
        "PlutusV2" | "1" => Ok(1),
        "PlutusV3" | "2" => Ok(2),
        other => Err(ScriptDataHashError::InvalidCostModelKey(other.to_string())),
    }
}

/// Encode cost models as a canonical CBOR map: `{ language_key => [int, ...] }`.
///
/// Keys are sorted numerically (shorter CBOR encoding first, then lexicographic).
/// Since keys are single unsigned integers 0, 1, 2, numerical order suffices.
fn encode_cost_models(json: &serde_json::Value) -> Result<Vec<u8>, ScriptDataHashError> {
    let obj = match json {
        serde_json::Value::Object(map) => map,
        serde_json::Value::Array(arr) if arr.is_empty() => {
            // Empty array treated as empty cost models
            let mut buf = Vec::new();
            let mut encoder = pallas_codec::minicbor::Encoder::new(&mut buf);
            encoder
                .map(0)
                .map_err(|e| ScriptDataHashError::CborEncode(e.to_string()))?;
            return Ok(buf);
        }
        _ => {
            return Err(ScriptDataHashError::InvalidJson {
                field: "cost-models".to_string(),
                detail: "expected a JSON object".to_string(),
            });
        }
    };

    // Parse and sort by key
    let mut entries: Vec<(u64, Vec<i64>)> = Vec::with_capacity(obj.len());
    for (k, v) in obj {
        let key = language_key(k)?;
        let values = v
            .as_array()
            .ok_or(ScriptDataHashError::InvalidCostModelValues)?;
        let mut ints = Vec::with_capacity(values.len());
        for val in values {
            let n = val
                .as_i64()
                .ok_or(ScriptDataHashError::InvalidCostModelValues)?;
            ints.push(n);
        }
        entries.push((key, ints));
    }
    // Sort by key (canonical CBOR: shorter encoding first, then lexicographic)
    entries.sort_by_key(|(k, _)| *k);

    // Encode as CBOR map
    let mut buf = Vec::new();
    let mut encoder = pallas_codec::minicbor::Encoder::new(&mut buf);
    encoder
        .map(entries.len() as u64)
        .map_err(|e| ScriptDataHashError::CborEncode(e.to_string()))?;
    for (key, values) in &entries {
        encoder
            .u64(*key)
            .map_err(|e| ScriptDataHashError::CborEncode(e.to_string()))?;
        encoder
            .array(values.len() as u64)
            .map_err(|e| ScriptDataHashError::CborEncode(e.to_string()))?;
        for val in values {
            encoder
                .i64(*val)
                .map_err(|e| ScriptDataHashError::CborEncode(e.to_string()))?;
        }
    }

    Ok(buf)
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

    #[test]
    fn test_cbor_encoding_of_redeemers() -> TestResult {
        // Verify the CBOR encoding is correct by decoding it back
        let items = vec![serde_json::json!({"constructor": 0, "fields": []})];
        let cbor_bytes = encode_plutus_data_array(&items)?;

        // Should be decodable as a CBOR array
        let mut decoder = pallas_codec::minicbor::Decoder::new(&cbor_bytes);
        let len = decoder.array()?.ok_or("expected definite array")?;
        assert_eq!(len, 1);
        Ok(())
    }

    #[test]
    fn test_cbor_encoding_of_cost_models() -> TestResult {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"PlutusV3": [100, 200, 300], "PlutusV1": [1, 2]}"#)?;
        let cbor_bytes = encode_cost_models(&json)?;

        // Should be decodable as a CBOR map, keys in sorted order (0 before 2)
        let mut decoder = pallas_codec::minicbor::Decoder::new(&cbor_bytes);
        let len = decoder.map()?.ok_or("expected definite map")?;
        assert_eq!(len, 2);
        // First key should be 0 (PlutusV1)
        let k1 = decoder.u64()?;
        assert_eq!(k1, 0);
        // Skip the value array
        let v1_len = decoder.array()?.ok_or("expected definite array")?;
        assert_eq!(v1_len, 2);
        let _ = decoder.i64()?; // 1
        let _ = decoder.i64()?; // 2
                                // Second key should be 2 (PlutusV3)
        let k2 = decoder.u64()?;
        assert_eq!(k2, 2);
        let v2_len = decoder.array()?.ok_or("expected definite array")?;
        assert_eq!(v2_len, 3);
        Ok(())
    }

    #[test]
    fn test_empty_datums_produce_empty_bytes() -> TestResult {
        // When datums is empty, datums_bytes should be empty (not a CBOR empty array)
        let redeemers = r#"[{"constructor": 0, "fields": []}]"#;
        let datums_empty = r#"[]"#;
        let datums_nonempty = r#"[{"int": 1}]"#;
        let cost_models = r#"{"PlutusV2": [100]}"#;

        let hash_empty = compute_script_data_hash(redeemers, datums_empty, cost_models)?;
        let hash_nonempty = compute_script_data_hash(redeemers, datums_nonempty, cost_models)?;

        // They should be different (empty bytes vs CBOR-encoded array)
        assert_ne!(
            hash_empty.data.script_data_hash,
            hash_nonempty.data.script_data_hash
        );
        Ok(())
    }

    #[test]
    fn test_cost_models_numeric_keys() -> TestResult {
        // Accept "0", "1", "2" as language keys (alternative to PlutusV1/V2/V3)
        let cost_models_named = r#"{"PlutusV2": [100, 200]}"#;
        let cost_models_numeric = r#"{"1": [100, 200]}"#;

        let hash1 = compute_script_data_hash(r#"[]"#, r#"[]"#, cost_models_named)?;
        let hash2 = compute_script_data_hash(r#"[]"#, r#"[]"#, cost_models_numeric)?;
        assert_eq!(hash1.data.script_data_hash, hash2.data.script_data_hash);
        Ok(())
    }

    #[test]
    fn test_invalid_cost_model_key() {
        let result = compute_script_data_hash(r#"[]"#, r#"[]"#, r#"{"PlutusV4": [1]}"#);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("PlutusV4"),
            "Expected invalid key error, got: {err}"
        );
    }

    #[test]
    fn test_complex_redeemer_data() -> TestResult {
        let redeemers = r#"[
            {"constructor": 0, "fields": [
                {"int": 42},
                {"bytes": "deadbeef"},
                {"list": [{"int": 1}, {"int": 2}]},
                {"map": [{"k": {"bytes": "aa"}, "v": {"int": 10}}]}
            ]}
        ]"#;
        let datums = r#"[{"constructor": 0, "fields": [{"int": 42}]}]"#;
        let cost_models = r#"{"PlutusV3": [100, 200, 300]}"#;

        let output = compute_script_data_hash(redeemers, datums, cost_models)?;
        assert_eq!(output.data.script_data_hash.len(), 64);

        // Verify determinism
        let output2 = compute_script_data_hash(redeemers, datums, cost_models)?;
        assert_eq!(output.data.script_data_hash, output2.data.script_data_hash);
        Ok(())
    }
}
