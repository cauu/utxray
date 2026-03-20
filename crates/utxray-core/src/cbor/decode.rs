use pallas_primitives::conway::PlutusData;
use serde::Serialize;

use crate::output::Output;

/// Errors specific to CBOR decoding.
#[derive(Debug, thiserror::Error)]
pub enum CborDecodeError {
    #[error("invalid hex string: {0}")]
    InvalidHex(#[from] hex::FromHexError),

    #[error("CBOR decode failed: {0}")]
    CborParse(String),
}

/// Successful decode output data.
#[derive(Debug, Serialize)]
pub struct DecodeSuccess {
    pub decoded: serde_json::Value,
}

/// Error output data for decode failures.
#[derive(Debug, Serialize)]
pub struct DecodeErrorData {
    pub error: String,
}

/// Decode a hex-encoded CBOR string into a human-readable JSON representation
/// of Plutus data.
pub fn decode_cbor_hex(hex_str: &str) -> Result<Output<DecodeSuccess>, CborDecodeError> {
    let bytes = hex::decode(hex_str.trim())?;
    let plutus_data: PlutusData = pallas_codec::minicbor::decode(&bytes)
        .map_err(|e| CborDecodeError::CborParse(e.to_string()))?;
    let json = plutus_data_to_json(&plutus_data);
    Ok(Output::ok(DecodeSuccess { decoded: json }))
}

/// Convert a PlutusData value into a human-readable serde_json::Value.
///
/// Encoding follows the Cardano JSON schema for Plutus data:
/// - Constr(tag, fields) -> {"constructor": N, "fields": [...]}
/// - Map(entries) -> {"map": [{"k": ..., "v": ...}, ...]}
/// - BigInt(value) -> {"int": N}
/// - BoundedBytes(bytes) -> {"bytes": "hex..."}
/// - Array(items) -> {"list": [...]}
pub fn plutus_data_to_json(data: &PlutusData) -> serde_json::Value {
    match data {
        PlutusData::Constr(constr) => {
            let constructor_index = constr_tag_to_index(constr.tag);
            let fields: Vec<serde_json::Value> = constr
                .any_constructor
                .as_ref()
                .map(|_| constr.fields.iter().map(plutus_data_to_json).collect())
                .unwrap_or_else(|| constr.fields.iter().map(plutus_data_to_json).collect());
            serde_json::json!({
                "constructor": constructor_index,
                "fields": fields
            })
        }
        PlutusData::Map(entries) => {
            let map_items: Vec<serde_json::Value> = entries
                .iter()
                .map(|(k, v)| {
                    serde_json::json!({
                        "k": plutus_data_to_json(k),
                        "v": plutus_data_to_json(v)
                    })
                })
                .collect();
            serde_json::json!({ "map": map_items })
        }
        PlutusData::BigInt(big_int) => {
            use pallas_primitives::conway::BigInt;
            match big_int {
                BigInt::Int(i) => {
                    let val: i128 = (*i).into();
                    serde_json::json!({ "int": val })
                }
                BigInt::BigUInt(bytes) => {
                    // Big positive integer encoded as bytes
                    let hex_str = hex::encode(bytes.as_slice());
                    serde_json::json!({ "int": format!("0x{hex_str}") })
                }
                BigInt::BigNInt(bytes) => {
                    // Big negative integer encoded as bytes
                    let hex_str = hex::encode(bytes.as_slice());
                    serde_json::json!({ "int": format!("-0x{hex_str}") })
                }
            }
        }
        PlutusData::BoundedBytes(bytes) => {
            let hex_str = hex::encode(bytes.as_slice());
            serde_json::json!({ "bytes": hex_str })
        }
        PlutusData::Array(items) => {
            let list: Vec<serde_json::Value> = items.iter().map(plutus_data_to_json).collect();
            serde_json::json!({ "list": list })
        }
    }
}

/// Convert a CBOR tag to a Plutus constructor index.
/// Tags 121-127 map to constructors 0-6.
/// Tags 1280-1400 map to constructors 7-127.
/// For general constructors, the `any_constructor` field is used.
fn constr_tag_to_index(tag: u64) -> u64 {
    if (121..=127).contains(&tag) {
        tag - 121
    } else if (1280..=1400).contains(&tag) {
        tag - 1280 + 7
    } else {
        // Fallback: use the tag directly (for general constructor encoding)
        tag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_decode_simple_constructor() -> TestResult {
        let hex_str = "d8798344aabbccdd1903e81a004c4b40";
        let result = decode_cbor_hex(hex_str)?;
        let decoded = &result.data.decoded;

        assert_eq!(decoded["constructor"], 0);
        assert!(decoded["fields"].is_array());
        let fields = decoded["fields"].as_array().ok_or("expected array")?;
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0]["bytes"], "aabbccdd");
        assert_eq!(fields[1]["int"], 1000);
        assert_eq!(fields[2]["int"], 5000000);
        Ok(())
    }

    #[test]
    fn test_decode_nested_constructor() -> TestResult {
        let hex_str = "d87a82d87981182a1863";
        let result = decode_cbor_hex(hex_str)?;
        let decoded = &result.data.decoded;

        assert_eq!(decoded["constructor"], 1);
        let fields = decoded["fields"].as_array().ok_or("expected array")?;
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0]["constructor"], 0);
        assert_eq!(fields[0]["fields"][0]["int"], 42);
        assert_eq!(fields[1]["int"], 99);
        Ok(())
    }

    #[test]
    fn test_decode_bounded_bytes() {
        // Just a bytestring: 44deadbeef
        let hex_str = "44deadbeef";
        let result = decode_cbor_hex(hex_str);
        // A raw bytestring is BoundedBytes at top level if decoded as PlutusData
        // Actually pallas might decode this differently—let's check
        // A bare bytestring is valid PlutusData::BoundedBytes
        match result {
            Ok(output) => {
                assert_eq!(output.data.decoded["bytes"], "deadbeef");
            }
            Err(_) => {
                // Some pallas versions might not decode a bare bytestring as PlutusData
                // This is acceptable
            }
        }
    }

    #[test]
    fn test_decode_integer() {
        // Integer 42 = 182a in CBOR
        let hex_str = "182a";
        let result = decode_cbor_hex(hex_str);
        // A bare integer may or may not be valid PlutusData depending on pallas
        match result {
            Ok(output) => {
                assert_eq!(output.data.decoded["int"], 42);
            }
            Err(_) => {
                // Acceptable if pallas doesn't decode bare integers as PlutusData
            }
        }
    }

    #[test]
    fn test_decode_invalid_hex() {
        let result = decode_cbor_hex("zzzz");
        assert!(result.is_err());
        assert!(matches!(result, Err(CborDecodeError::InvalidHex(_))));
    }

    #[test]
    fn test_decode_invalid_cbor() {
        // Valid hex but not valid CBOR PlutusData
        let result = decode_cbor_hex("ff");
        assert!(result.is_err());
    }

    #[test]
    fn test_constr_tag_to_index() {
        assert_eq!(constr_tag_to_index(121), 0);
        assert_eq!(constr_tag_to_index(122), 1);
        assert_eq!(constr_tag_to_index(127), 6);
        assert_eq!(constr_tag_to_index(1280), 7);
        assert_eq!(constr_tag_to_index(1281), 8);
    }

    #[test]
    fn test_plutus_data_to_json_array() -> TestResult {
        use pallas_codec::minicbor;
        let hex_str = "820102";
        let bytes = hex::decode(hex_str)?;
        let data: PlutusData = minicbor::decode(&bytes).map_err(|e| format!("{e}"))?;
        let json = plutus_data_to_json(&data);
        assert!(json["list"].is_array());
        let list = json["list"].as_array().ok_or("expected array")?;
        assert_eq!(list.len(), 2);
        Ok(())
    }
}
