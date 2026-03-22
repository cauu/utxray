use serde::Serialize;

use crate::cbor::decode::{decode_cbor_hex, CborDecodeError};
use crate::output::Output;

/// Error types for CBOR diff operations.
#[derive(Debug, thiserror::Error)]
pub enum CborDiffError {
    #[error("left input: {0}")]
    LeftDecode(CborDecodeError),

    #[error("right input: {0}")]
    RightDecode(CborDecodeError),
}

/// A single difference found between two CBOR values.
#[derive(Debug, Serialize)]
pub struct Difference {
    pub path: String,
    pub left: serde_json::Value,
    pub right: serde_json::Value,
    #[serde(rename = "type")]
    pub diff_type: String,
}

/// Summary of a decoded CBOR value.
#[derive(Debug, Serialize)]
pub struct ValueSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constructor: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_count: Option<usize>,
    pub kind: String,
}

/// The output data for a CBOR diff operation.
#[derive(Debug, Serialize)]
pub struct CborDiffOutput {
    pub identical: bool,
    pub differences: Vec<Difference>,
    pub left_summary: ValueSummary,
    pub right_summary: ValueSummary,
}

/// Error output data for diff failures.
#[derive(Debug, Serialize)]
pub struct CborDiffErrorData {
    pub error: String,
}

/// Compute a structural diff between two CBOR hex-encoded PlutusData values.
///
/// Both inputs are decoded to JSON, then recursively compared.
pub fn diff_cbor_hex(
    left_hex: &str,
    right_hex: &str,
) -> Result<Output<CborDiffOutput>, CborDiffError> {
    let left_output = decode_cbor_hex(left_hex).map_err(CborDiffError::LeftDecode)?;
    let right_output = decode_cbor_hex(right_hex).map_err(CborDiffError::RightDecode)?;

    let left_json = &left_output.data.decoded;
    let right_json = &right_output.data.decoded;

    let mut differences = Vec::new();
    diff_values("$", left_json, right_json, &mut differences);

    let left_summary = summarize_value(left_json);
    let right_summary = summarize_value(right_json);

    let identical = differences.is_empty();
    let output = CborDiffOutput {
        identical,
        differences,
        left_summary,
        right_summary,
    };

    Ok(Output::ok(output))
}

/// Recursively diff two JSON values and collect differences.
fn diff_values(
    path: &str,
    left: &serde_json::Value,
    right: &serde_json::Value,
    diffs: &mut Vec<Difference>,
) {
    if left == right {
        return;
    }

    match (left, right) {
        // Both are objects: compare keys
        (serde_json::Value::Object(left_map), serde_json::Value::Object(right_map)) => {
            // Keys in left but not in right -> removed
            for (key, left_val) in left_map {
                let child_path = format!("{path}.{key}");
                match right_map.get(key) {
                    Some(right_val) => {
                        diff_values(&child_path, left_val, right_val, diffs);
                    }
                    None => {
                        diffs.push(Difference {
                            path: child_path,
                            left: left_val.clone(),
                            right: serde_json::Value::Null,
                            diff_type: "removed".to_string(),
                        });
                    }
                }
            }
            // Keys in right but not in left -> added
            for (key, right_val) in right_map {
                if !left_map.contains_key(key) {
                    let child_path = format!("{path}.{key}");
                    diffs.push(Difference {
                        path: child_path,
                        left: serde_json::Value::Null,
                        right: right_val.clone(),
                        diff_type: "added".to_string(),
                    });
                }
            }
        }
        // Both are arrays: compare element-by-element
        (serde_json::Value::Array(left_arr), serde_json::Value::Array(right_arr)) => {
            let max_len = left_arr.len().max(right_arr.len());
            for i in 0..max_len {
                let child_path = format!("{path}[{i}]");
                match (left_arr.get(i), right_arr.get(i)) {
                    (Some(l), Some(r)) => {
                        diff_values(&child_path, l, r, diffs);
                    }
                    (Some(l), None) => {
                        diffs.push(Difference {
                            path: child_path,
                            left: l.clone(),
                            right: serde_json::Value::Null,
                            diff_type: "removed".to_string(),
                        });
                    }
                    (None, Some(r)) => {
                        diffs.push(Difference {
                            path: child_path,
                            left: serde_json::Value::Null,
                            right: r.clone(),
                            diff_type: "added".to_string(),
                        });
                    }
                    (None, None) => {}
                }
            }
        }
        // Different types or different scalar values
        _ => {
            diffs.push(Difference {
                path: path.to_string(),
                left: left.clone(),
                right: right.clone(),
                diff_type: "value_changed".to_string(),
            });
        }
    }
}

/// Produce a summary of a decoded PlutusData JSON value.
fn summarize_value(value: &serde_json::Value) -> ValueSummary {
    if let Some(obj) = value.as_object() {
        if let Some(constructor) = obj.get("constructor") {
            let field_count = obj
                .get("fields")
                .and_then(|f| f.as_array())
                .map(|a| a.len());
            return ValueSummary {
                constructor: constructor.as_i64(),
                field_count,
                kind: "constructor".to_string(),
            };
        }
        if let Some(list) = obj.get("list") {
            return ValueSummary {
                constructor: None,
                field_count: list.as_array().map(|a| a.len()),
                kind: "list".to_string(),
            };
        }
        if obj.contains_key("map") {
            let entry_count = obj.get("map").and_then(|m| m.as_array()).map(|a| a.len());
            return ValueSummary {
                constructor: None,
                field_count: entry_count,
                kind: "map".to_string(),
            };
        }
        if obj.contains_key("int") {
            return ValueSummary {
                constructor: None,
                field_count: None,
                kind: "int".to_string(),
            };
        }
        if obj.contains_key("bytes") {
            return ValueSummary {
                constructor: None,
                field_count: None,
                kind: "bytes".to_string(),
            };
        }
    }
    ValueSummary {
        constructor: None,
        field_count: None,
        kind: "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_identical_values() -> TestResult {
        // Same CBOR hex -> identical: true
        let hex = "d8798344aabbccdd1903e81a004c4b40";
        let result = diff_cbor_hex(hex, hex)?;
        assert!(result.data.identical);
        assert!(result.data.differences.is_empty());
        assert_eq!(result.data.left_summary.kind, "constructor");
        assert_eq!(result.data.left_summary.constructor, Some(0));
        assert_eq!(result.data.left_summary.field_count, Some(3));
        Ok(())
    }

    #[test]
    fn test_different_values() -> TestResult {
        // Constr(0, [bytes, 1000, 5000000]) vs Constr(1, [Constr(0, [42]), 99])
        let left = "d8798344aabbccdd1903e81a004c4b40";
        let right = "d87a82d87981182a1863";
        let result = diff_cbor_hex(left, right)?;
        assert!(!result.data.identical);
        assert!(!result.data.differences.is_empty());
        assert_eq!(result.data.left_summary.constructor, Some(0));
        assert_eq!(result.data.right_summary.constructor, Some(1));
        Ok(())
    }

    #[test]
    fn test_different_field_count() -> TestResult {
        // Constr(0, [bytes, 1000, 5000000]) has 3 fields
        // Constr(1, [Constr(0, [42]), 99]) has 2 fields
        let left = "d8798344aabbccdd1903e81a004c4b40";
        let right = "d87a82d87981182a1863";
        let result = diff_cbor_hex(left, right)?;
        assert_eq!(result.data.left_summary.field_count, Some(3));
        assert_eq!(result.data.right_summary.field_count, Some(2));
        Ok(())
    }

    #[test]
    fn test_invalid_left_hex() {
        let result = diff_cbor_hex("zzzz", "d8798344aabbccdd1903e81a004c4b40");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("left input"));
    }

    #[test]
    fn test_invalid_right_hex() {
        let result = diff_cbor_hex("d8798344aabbccdd1903e81a004c4b40", "zzzz");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("right input"));
    }

    #[test]
    fn test_diff_value_changed_type() -> TestResult {
        // Same constructor but different field values
        // Constr(0, [bytes(aabbccdd), 1000, 5000000]) vs Constr(0, [bytes(aabbccdd), 1000, 6000000])
        // Construct manually: d87983 44aabbccdd 1903e8 1a005b8d80 (6000000 = 0x5B8D80)
        let left = "d8798344aabbccdd1903e81a004c4b40";
        let right = "d8798344aabbccdd1903e81a005b8d80";
        let result = diff_cbor_hex(left, right)?;
        assert!(!result.data.identical);

        // Should have exactly one difference at the third field's int value
        let value_changed: Vec<_> = result
            .data
            .differences
            .iter()
            .filter(|d| d.diff_type == "value_changed")
            .collect();
        assert!(!value_changed.is_empty());
        Ok(())
    }

    #[test]
    fn test_summarize_int() {
        let val = serde_json::json!({"int": 42});
        let summary = summarize_value(&val);
        assert_eq!(summary.kind, "int");
        assert!(summary.constructor.is_none());
    }

    #[test]
    fn test_summarize_bytes() {
        let val = serde_json::json!({"bytes": "aabb"});
        let summary = summarize_value(&val);
        assert_eq!(summary.kind, "bytes");
    }
}
