use pallas_codec::utils::{Int, KeyValuePairs};
use pallas_primitives::alonzo::BoundedBytes;
use pallas_primitives::conway::{BigInt, Constr, PlutusData};

/// Errors specific to JSON-to-PlutusData conversion.
#[derive(Debug, thiserror::Error)]
pub enum PlutusDataEncodeError {
    #[error("invalid PlutusData JSON: {0}")]
    InvalidStructure(String),

    #[error("hex decode failed: {0}")]
    HexDecode(#[from] hex::FromHexError),

    #[error("integer out of range: {0}")]
    IntOutOfRange(String),

    #[error("CBOR encoding failed: {0}")]
    CborEncode(String),
}

/// Convert a Cardano JSON schema PlutusData value into a `PlutusData`.
///
/// Supported forms:
/// - `{"constructor": N, "fields": [...]}` -> `PlutusData::Constr(tag, fields)`
/// - `{"int": N}` -> `PlutusData::BigInt(Int(N))`
/// - `{"bytes": "hex"}` -> `PlutusData::BoundedBytes(bytes)`
/// - `{"list": [...]}` -> `PlutusData::Array(items)`
/// - `{"map": [{"k": ..., "v": ...}, ...]}` -> `PlutusData::Map(entries)`
pub fn json_to_plutus_data(json: &serde_json::Value) -> Result<PlutusData, PlutusDataEncodeError> {
    let obj = json.as_object().ok_or_else(|| {
        PlutusDataEncodeError::InvalidStructure("expected a JSON object".to_string())
    })?;

    if let Some(constructor_val) = obj.get("constructor") {
        // Constr
        let index = constructor_val.as_u64().ok_or_else(|| {
            PlutusDataEncodeError::InvalidStructure(
                "\"constructor\" must be a non-negative integer".to_string(),
            )
        })?;

        let fields_val = obj.get("fields").ok_or_else(|| {
            PlutusDataEncodeError::InvalidStructure(
                "constructor requires \"fields\" array".to_string(),
            )
        })?;
        let fields_arr = fields_val.as_array().ok_or_else(|| {
            PlutusDataEncodeError::InvalidStructure("\"fields\" must be an array".to_string())
        })?;

        let mut fields = Vec::with_capacity(fields_arr.len());
        for f in fields_arr {
            fields.push(json_to_plutus_data(f)?);
        }

        let (tag, any_constructor) = index_to_constr_tag(index);

        Ok(PlutusData::Constr(Constr {
            tag,
            any_constructor,
            fields,
        }))
    } else if let Some(int_val) = obj.get("int") {
        // BigInt
        let n = int_val.as_i64().ok_or_else(|| {
            // Try as i128 via string
            PlutusDataEncodeError::InvalidStructure(format!(
                "\"int\" value {} is not a valid i64",
                int_val
            ))
        })?;
        let int = Int::from(n);
        Ok(PlutusData::BigInt(BigInt::Int(int)))
    } else if let Some(bytes_val) = obj.get("bytes") {
        // BoundedBytes
        let hex_str = bytes_val.as_str().ok_or_else(|| {
            PlutusDataEncodeError::InvalidStructure("\"bytes\" must be a hex string".to_string())
        })?;
        let bytes = hex::decode(hex_str)?;
        Ok(PlutusData::BoundedBytes(BoundedBytes::from(bytes)))
    } else if let Some(list_val) = obj.get("list") {
        // Array
        let arr = list_val.as_array().ok_or_else(|| {
            PlutusDataEncodeError::InvalidStructure("\"list\" must be an array".to_string())
        })?;
        let mut items = Vec::with_capacity(arr.len());
        for item in arr {
            items.push(json_to_plutus_data(item)?);
        }
        Ok(PlutusData::Array(items))
    } else if let Some(map_val) = obj.get("map") {
        // Map
        let arr = map_val.as_array().ok_or_else(|| {
            PlutusDataEncodeError::InvalidStructure("\"map\" must be an array".to_string())
        })?;
        let mut entries = Vec::with_capacity(arr.len());
        for entry in arr {
            let entry_obj = entry.as_object().ok_or_else(|| {
                PlutusDataEncodeError::InvalidStructure(
                    "map entry must be an object with \"k\" and \"v\"".to_string(),
                )
            })?;
            let k = entry_obj.get("k").ok_or_else(|| {
                PlutusDataEncodeError::InvalidStructure("map entry missing \"k\" key".to_string())
            })?;
            let v = entry_obj.get("v").ok_or_else(|| {
                PlutusDataEncodeError::InvalidStructure("map entry missing \"v\" key".to_string())
            })?;
            entries.push((json_to_plutus_data(k)?, json_to_plutus_data(v)?));
        }
        Ok(PlutusData::Map(KeyValuePairs::from(entries)))
    } else {
        Err(PlutusDataEncodeError::InvalidStructure(format!(
            "unrecognized PlutusData JSON structure: {}",
            serde_json::to_string(json).unwrap_or_else(|_| "<invalid>".to_string())
        )))
    }
}

/// Encode a `PlutusData` value to CBOR bytes.
pub fn plutus_data_to_cbor(data: &PlutusData) -> Result<Vec<u8>, PlutusDataEncodeError> {
    pallas_codec::minicbor::to_vec(data)
        .map_err(|e| PlutusDataEncodeError::CborEncode(e.to_string()))
}

/// Convert a constructor index (0, 1, ...) to the CBOR tag used by pallas.
///
/// Constructors 0-6 use tags 121-127.
/// Constructors 7-127 use tags 1280-1400.
/// Constructors > 127 use the general encoding (tag 102 with any_constructor).
fn index_to_constr_tag(index: u64) -> (u64, Option<u64>) {
    if index <= 6 {
        (121 + index, None)
    } else if index <= 127 {
        (1280 + index - 7, None)
    } else {
        // General constructor encoding uses tag 102
        (102, Some(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cbor::decode::plutus_data_to_json;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_roundtrip_constructor() -> TestResult {
        let json_str = r#"{"constructor": 0, "fields": [{"int": 42}, {"bytes": "aabb"}]}"#;
        let json: serde_json::Value = serde_json::from_str(json_str)?;
        let data = json_to_plutus_data(&json)?;

        // Encode to CBOR and decode back
        let cbor_bytes = plutus_data_to_cbor(&data)?;
        let decoded: PlutusData = pallas_codec::minicbor::decode(&cbor_bytes)
            .map_err(|e| format!("decode failed: {e}"))?;
        let roundtrip_json = plutus_data_to_json(&decoded);

        assert_eq!(roundtrip_json["constructor"], 0);
        let fields = roundtrip_json["fields"].as_array().ok_or("expected fields array")?;
        assert_eq!(fields[0]["int"], 42);
        assert_eq!(fields[1]["bytes"], "aabb");
        Ok(())
    }

    #[test]
    fn test_roundtrip_integer() -> TestResult {
        let json: serde_json::Value = serde_json::from_str(r#"{"int": -100}"#)?;
        let data = json_to_plutus_data(&json)?;
        let cbor_bytes = plutus_data_to_cbor(&data)?;
        let decoded: PlutusData = pallas_codec::minicbor::decode(&cbor_bytes)
            .map_err(|e| format!("decode failed: {e}"))?;
        let roundtrip_json = plutus_data_to_json(&decoded);
        assert_eq!(roundtrip_json["int"], -100);
        Ok(())
    }

    #[test]
    fn test_roundtrip_bytes() -> TestResult {
        let json: serde_json::Value = serde_json::from_str(r#"{"bytes": "deadbeef"}"#)?;
        let data = json_to_plutus_data(&json)?;
        let cbor_bytes = plutus_data_to_cbor(&data)?;
        let decoded: PlutusData = pallas_codec::minicbor::decode(&cbor_bytes)
            .map_err(|e| format!("decode failed: {e}"))?;
        let roundtrip_json = plutus_data_to_json(&decoded);
        assert_eq!(roundtrip_json["bytes"], "deadbeef");
        Ok(())
    }

    #[test]
    fn test_roundtrip_list() -> TestResult {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"list": [{"int": 1}, {"int": 2}, {"int": 3}]}"#)?;
        let data = json_to_plutus_data(&json)?;
        let cbor_bytes = plutus_data_to_cbor(&data)?;
        let decoded: PlutusData = pallas_codec::minicbor::decode(&cbor_bytes)
            .map_err(|e| format!("decode failed: {e}"))?;
        let roundtrip_json = plutus_data_to_json(&decoded);
        let list = roundtrip_json["list"].as_array().ok_or("expected list array")?;
        assert_eq!(list.len(), 3);
        assert_eq!(list[0]["int"], 1);
        Ok(())
    }

    #[test]
    fn test_roundtrip_map() -> TestResult {
        let json: serde_json::Value =
            serde_json::from_str(r#"{"map": [{"k": {"bytes": "aa"}, "v": {"int": 10}}]}"#)?;
        let data = json_to_plutus_data(&json)?;
        let cbor_bytes = plutus_data_to_cbor(&data)?;
        let decoded: PlutusData = pallas_codec::minicbor::decode(&cbor_bytes)
            .map_err(|e| format!("decode failed: {e}"))?;
        let roundtrip_json = plutus_data_to_json(&decoded);
        let map_entries = roundtrip_json["map"].as_array().ok_or("expected map array")?;
        assert_eq!(map_entries.len(), 1);
        assert_eq!(map_entries[0]["k"]["bytes"], "aa");
        assert_eq!(map_entries[0]["v"]["int"], 10);
        Ok(())
    }

    #[test]
    fn test_roundtrip_nested_constructor() -> TestResult {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"constructor": 1, "fields": [{"constructor": 0, "fields": [{"int": 42}]}, {"int": 99}]}"#,
        )?;
        let data = json_to_plutus_data(&json)?;
        let cbor_bytes = plutus_data_to_cbor(&data)?;
        let decoded: PlutusData = pallas_codec::minicbor::decode(&cbor_bytes)
            .map_err(|e| format!("decode failed: {e}"))?;
        let roundtrip_json = plutus_data_to_json(&decoded);
        assert_eq!(roundtrip_json["constructor"], 1);
        let fields = roundtrip_json["fields"].as_array().ok_or("expected fields array")?;
        assert_eq!(fields[0]["constructor"], 0);
        assert_eq!(fields[0]["fields"][0]["int"], 42);
        assert_eq!(fields[1]["int"], 99);
        Ok(())
    }

    #[test]
    fn test_constructor_high_index() -> TestResult {
        // Constructor index 7 uses tag 1280
        let json: serde_json::Value = serde_json::from_str(r#"{"constructor": 7, "fields": []}"#)?;
        let data = json_to_plutus_data(&json)?;
        let cbor_bytes = plutus_data_to_cbor(&data)?;
        let decoded: PlutusData = pallas_codec::minicbor::decode(&cbor_bytes)
            .map_err(|e| format!("decode failed: {e}"))?;
        let roundtrip_json = plutus_data_to_json(&decoded);
        assert_eq!(roundtrip_json["constructor"], 7);
        Ok(())
    }

    #[test]
    fn test_invalid_structure() {
        let json: serde_json::Value = serde_json::json!({"unknown": 42});
        let result = json_to_plutus_data(&json);
        assert!(result.is_err());
    }

    #[test]
    fn test_non_object() {
        let json: serde_json::Value = serde_json::json!(42);
        let result = json_to_plutus_data(&json);
        assert!(result.is_err());
    }

    #[test]
    fn test_index_to_constr_tag_values() {
        assert_eq!(index_to_constr_tag(0), (121, None));
        assert_eq!(index_to_constr_tag(6), (127, None));
        assert_eq!(index_to_constr_tag(7), (1280, None));
        assert_eq!(index_to_constr_tag(127), (1400, None));
        assert_eq!(index_to_constr_tag(128), (102, Some(128)));
    }
}
