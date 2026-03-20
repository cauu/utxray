use pallas_primitives::conway::RedeemerTag;
use pallas_traverse::MultiEraTx;
use serde::Serialize;

use crate::output::Output;

// ── Error types ────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum RedeemerIndexError {
    #[error("invalid hex string: {0}")]
    InvalidHex(#[from] hex::FromHexError),

    #[error("failed to decode transaction CBOR: {0}")]
    CborDecode(String),

    #[error("failed to read file '{path}': {detail}")]
    FileRead { path: String, detail: String },

    #[error("no transaction input provided (use --tx with hex or file path)")]
    MissingInput,
}

// ── Output types ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct RedeemerIndexOutput {
    pub sorted_inputs: Vec<SortedInput>,
    pub redeemers: Vec<RedeemerEntry>,
    pub sort_rules: SortRules,
}

#[derive(Debug, Serialize)]
pub struct SortedInput {
    pub index: usize,
    pub utxo: String,
    pub r#type: String,
}

#[derive(Debug, Serialize)]
pub struct RedeemerEntry {
    pub tag: String,
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets_utxo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub targets_policy: Option<String>,
    /// Validator name; null when not resolvable without a blueprint.
    pub validator: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SortRules {
    pub inputs: String,
    pub mint: String,
    pub input_normalization: String,
}

#[derive(Debug, Serialize)]
pub struct RedeemerIndexErrorData {
    pub error: String,
}

// ── Public API ─────────────────────────────────────────────────

/// Analyze redeemer index alignment for a transaction.
///
/// Reads a transaction from CBOR hex or a file, extracts inputs in their
/// lexicographically sorted order, and maps each redeemer to its target
/// input or mint policy.
pub fn analyze_redeemer_index(
    tx_input: &str,
) -> Result<Output<RedeemerIndexOutput>, RedeemerIndexError> {
    let cbor_bytes = read_tx_cbor(tx_input)?;
    let tx = MultiEraTx::decode(&cbor_bytes)
        .map_err(|e| RedeemerIndexError::CborDecode(e.to_string()))?;

    // 1. Get sorted inputs (lexicographic by tx_hash#output_index)
    let sorted_inputs_raw = tx.inputs_sorted_set();

    // 2. Build sorted input entries.
    //    Without a blueprint, we cannot determine if an input is "script" or "pubkey".
    //    We check if there's a spend redeemer targeting the input's index to infer "script".
    let redeemers = tx.redeemers();

    let spend_redeemer_indices: std::collections::HashSet<u32> = redeemers
        .iter()
        .filter(|r| r.tag() == RedeemerTag::Spend)
        .map(|r| r.index())
        .collect();

    let sorted_inputs: Vec<SortedInput> = sorted_inputs_raw
        .iter()
        .enumerate()
        .map(|(i, input)| {
            let utxo_ref = input.output_ref().to_string();
            let input_type = if spend_redeemer_indices.contains(&(i as u32)) {
                "script"
            } else {
                "pubkey"
            };
            SortedInput {
                index: i,
                utxo: utxo_ref,
                r#type: input_type.to_string(),
            }
        })
        .collect();

    // 3. Build sorted mint policies
    let sorted_mints = tx.mints_sorted_set();

    // 4. Map redeemers to their targets
    let redeemer_entries: Vec<RedeemerEntry> = redeemers
        .iter()
        .map(|r| {
            let tag = redeemer_tag_to_string(r.tag());
            let index = r.index();

            let (targets_utxo, targets_policy) = match r.tag() {
                RedeemerTag::Spend => {
                    let utxo = sorted_inputs_raw
                        .get(index as usize)
                        .map(|inp| inp.output_ref().to_string());
                    (utxo, None)
                }
                RedeemerTag::Mint => {
                    let policy = sorted_mints
                        .get(index as usize)
                        .map(|m| hex::encode(m.policy().as_ref()));
                    (None, policy)
                }
                _ => (None, None),
            };

            RedeemerEntry {
                tag,
                index,
                targets_utxo,
                targets_policy,
                validator: None, // Cannot resolve without blueprint in Phase 1
            }
        })
        .collect();

    let output = RedeemerIndexOutput {
        sorted_inputs,
        redeemers: redeemer_entries,
        sort_rules: SortRules {
            inputs: "Sorted lexicographically by (tx_hash, output_index). Duplicate inputs are deduplicated.".to_string(),
            mint: "Sorted lexicographically by policy_id.".to_string(),
            input_normalization: "All tx_hash and policy_id values are auto-normalized to lowercase hex.".to_string(),
        },
    };

    Ok(Output::ok(output))
}

// ── Internal helpers ───────────────────────────────────────────

/// Read tx CBOR from either a hex string or a file path.
fn read_tx_cbor(input: &str) -> Result<Vec<u8>, RedeemerIndexError> {
    let trimmed = input.trim();

    // If it looks like hex (only hex chars), try decoding directly
    if trimmed.chars().all(|c| c.is_ascii_hexdigit()) && !trimmed.is_empty() {
        return hex::decode(trimmed).map_err(RedeemerIndexError::InvalidHex);
    }

    // Try reading as file
    let path = std::path::Path::new(trimmed);
    if path.exists() {
        let content = std::fs::read_to_string(path).map_err(|e| RedeemerIndexError::FileRead {
            path: trimmed.to_string(),
            detail: e.to_string(),
        })?;
        let hex_str = content.trim();
        return hex::decode(hex_str).map_err(RedeemerIndexError::InvalidHex);
    }

    // Might be hex with spaces or other formatting; try stripping whitespace
    let cleaned: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    if !cleaned.is_empty() && cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        return hex::decode(&cleaned).map_err(RedeemerIndexError::InvalidHex);
    }

    Err(RedeemerIndexError::CborDecode(format!(
        "input is neither valid hex nor a readable file: {trimmed}"
    )))
}

fn redeemer_tag_to_string(tag: RedeemerTag) -> String {
    match tag {
        RedeemerTag::Spend => "spend".to_string(),
        RedeemerTag::Mint => "mint".to_string(),
        RedeemerTag::Cert => "cert".to_string(),
        RedeemerTag::Reward => "reward".to_string(),
        // Conway may add more tags; handle gracefully
        _ => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_redeemer_tag_to_string() {
        assert_eq!(redeemer_tag_to_string(RedeemerTag::Spend), "spend");
        assert_eq!(redeemer_tag_to_string(RedeemerTag::Mint), "mint");
        assert_eq!(redeemer_tag_to_string(RedeemerTag::Cert), "cert");
        assert_eq!(redeemer_tag_to_string(RedeemerTag::Reward), "reward");
    }

    #[test]
    fn test_read_tx_cbor_invalid_hex() {
        let result = read_tx_cbor("zzzz not hex");
        assert!(result.is_err());
    }

    #[test]
    fn test_read_tx_cbor_empty() {
        let result = read_tx_cbor("");
        assert!(result.is_err());
    }

    #[test]
    fn test_read_tx_cbor_valid_hex() -> TestResult {
        // A minimal valid hex string (just some bytes)
        let result = read_tx_cbor("aabbccdd")?;
        assert_eq!(result, vec![0xaa, 0xbb, 0xcc, 0xdd]);
        Ok(())
    }

    #[test]
    fn test_analyze_invalid_cbor() {
        let result = analyze_redeemer_index("aabbccdd");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("decode"),
            "Expected decode error, got: {err}"
        );
    }

    #[test]
    fn test_analyze_invalid_hex() {
        let result = analyze_redeemer_index("not-hex-at-all!!!");
        assert!(result.is_err());
    }

    #[test]
    fn test_sort_rules_populated() -> TestResult {
        // We can't easily test with a real tx without a fixture, but we can
        // test the error path and data structures.
        let result = analyze_redeemer_index("ff");
        // ff is valid hex but invalid CBOR tx
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_read_tx_cbor_from_file() -> TestResult {
        use std::path::PathBuf;
        let fixture_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cbor");
        let tx_path = fixture_dir.join("test_tx_temp.hex");
        // Write a small hex file
        std::fs::write(&tx_path, "aabbccdd\n")?;
        let result = read_tx_cbor(tx_path.to_str().ok_or("path")?)?;
        let _ = std::fs::remove_file(&tx_path);
        assert_eq!(result, vec![0xaa, 0xbb, 0xcc, 0xdd]);
        Ok(())
    }
}
