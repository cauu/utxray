//! Transaction signing: read an `.skey` file, sign the tx body hash,
//! and attach a VKeyWitness to the witness set.

use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use pallas_codec::utils::{Bytes, NonEmptySet};
use pallas_primitives::alonzo::VKeyWitness;
use pallas_primitives::conway::{Tx, WitnessSet};

/// Errors specific to transaction signing.
#[derive(Debug, thiserror::Error)]
pub enum SignError {
    #[error("failed to read signing key file '{path}': {source}")]
    ReadSkeyFile {
        path: String,
        source: std::io::Error,
    },

    #[error("invalid signing key file '{path}': {detail}")]
    InvalidSkeyFile { path: String, detail: String },

    #[error("invalid transaction CBOR: {0}")]
    InvalidTxCbor(String),

    #[error("ed25519 signing error: {0}")]
    SigningFailed(String),

    #[error("failed to encode signed transaction: {0}")]
    EncodeFailed(String),

    #[error("failed to write signed tx to '{path}': {source}")]
    WriteError {
        path: String,
        source: std::io::Error,
    },
}

/// Parsed signing key from a Cardano `.skey` JSON envelope.
pub struct ParsedSigningKey {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

// Manual Debug impl because SigningKey doesn't implement Debug in a useful way
// and we don't want to leak key material.
impl std::fmt::Debug for ParsedSigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParsedSigningKey")
            .field("verifying_key", &hex::encode(self.verifying_key.to_bytes()))
            .finish()
    }
}

/// Parse a Cardano `.skey` JSON file and extract the ed25519 signing key.
///
/// The expected format is:
/// ```json
/// {
///   "type": "PaymentSigningKeyShelley_ed25519",
///   "description": "...",
///   "cborHex": "5820<64-hex-chars>"
/// }
/// ```
pub fn parse_skey_file(path: &str) -> Result<ParsedSigningKey, SignError> {
    let contents = std::fs::read_to_string(path).map_err(|e| SignError::ReadSkeyFile {
        path: path.to_string(),
        source: e,
    })?;

    parse_skey_json(&contents, path)
}

/// Parse signing key from JSON content (testable without filesystem).
pub fn parse_skey_json(json_str: &str, path: &str) -> Result<ParsedSigningKey, SignError> {
    let parsed: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| SignError::InvalidSkeyFile {
            path: path.to_string(),
            detail: format!("invalid JSON: {e}"),
        })?;

    let cbor_hex = parsed
        .get("cborHex")
        .and_then(|v| v.as_str())
        .ok_or_else(|| SignError::InvalidSkeyFile {
            path: path.to_string(),
            detail: "missing 'cborHex' field".to_string(),
        })?;

    // The cborHex starts with "5820" (CBOR byte string tag for 32 bytes),
    // followed by 64 hex chars of the private key.
    if cbor_hex.len() < 68 {
        return Err(SignError::InvalidSkeyFile {
            path: path.to_string(),
            detail: format!(
                "cborHex too short (expected at least 68 hex chars, got {})",
                cbor_hex.len()
            ),
        });
    }

    let prefix = &cbor_hex[..4];
    if prefix != "5820" {
        return Err(SignError::InvalidSkeyFile {
            path: path.to_string(),
            detail: format!("cborHex does not start with '5820' (got '{prefix}')"),
        });
    }

    let key_hex = &cbor_hex[4..68];
    let key_bytes = hex::decode(key_hex).map_err(|e| SignError::InvalidSkeyFile {
        path: path.to_string(),
        detail: format!("invalid hex in private key: {e}"),
    })?;

    let key_array: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| SignError::InvalidSkeyFile {
            path: path.to_string(),
            detail: "private key must be exactly 32 bytes".to_string(),
        })?;

    let signing_key = SigningKey::from_bytes(&key_array);
    let verifying_key = signing_key.verifying_key();

    Ok(ParsedSigningKey {
        signing_key,
        verifying_key,
    })
}

/// Compute the transaction hash (blake2b-256 of the transaction body CBOR).
///
/// We decode as MintedTx to get the raw CBOR bytes of the body via KeepRaw,
/// which preserves the original encoding for correct hashing.
pub fn compute_tx_hash(tx_cbor: &[u8]) -> Result<[u8; 32], SignError> {
    // Decode as MintedTx to access KeepRaw body bytes
    let mtx: pallas_primitives::conway::MintedTx<'_> = pallas_codec::minicbor::decode(tx_cbor)
        .map_err(|e| SignError::InvalidTxCbor(format!("failed to decode transaction CBOR: {e}")))?;

    let body_raw = mtx.transaction_body.raw_cbor();

    let mut hasher = pallas_crypto::hash::Hasher::<256>::new();
    hasher.input(body_raw);
    let hash = hasher.finalize();

    Ok(*hash)
}

/// Sign a transaction with the given signing key and return the signed tx CBOR bytes.
///
/// Steps:
/// 1. Decode the transaction
/// 2. Compute tx hash (blake2b-256 of body CBOR)
/// 3. Sign the hash with ed25519
/// 4. Create VKeyWitness and add to witness set
/// 5. Re-encode the transaction
pub fn sign_transaction(tx_cbor: &[u8], skey_path: &str) -> Result<Vec<u8>, SignError> {
    let parsed_key = parse_skey_file(skey_path)?;
    sign_transaction_with_key(tx_cbor, &parsed_key)
}

/// Sign a transaction with an already-parsed key (useful for testing).
pub fn sign_transaction_with_key(
    tx_cbor: &[u8],
    parsed_key: &ParsedSigningKey,
) -> Result<Vec<u8>, SignError> {
    // 1. Compute tx hash from raw body bytes
    let tx_hash = compute_tx_hash(tx_cbor)?;

    // 2. Sign the hash
    let signature = parsed_key.signing_key.sign(&tx_hash);

    // 3. Build VKeyWitness
    let vkey_witness = VKeyWitness {
        vkey: Bytes::from(parsed_key.verifying_key.to_bytes().to_vec()),
        signature: Bytes::from(signature.to_bytes().to_vec()),
    };

    // 4. Decode the full Tx so we can modify the witness set
    let mut tx: Tx = pallas_codec::minicbor::decode(tx_cbor)
        .map_err(|e| SignError::InvalidTxCbor(format!("failed to decode transaction: {e}")))?;

    // 5. Add our VKeyWitness to the witness set
    add_vkey_witness(&mut tx.transaction_witness_set, vkey_witness);

    // 6. Re-encode
    let signed_bytes =
        pallas_codec::minicbor::to_vec(&tx).map_err(|e| SignError::EncodeFailed(format!("{e}")))?;

    Ok(signed_bytes)
}

/// Add a VKeyWitness to the witness set, appending to any existing witnesses.
fn add_vkey_witness(witness_set: &mut WitnessSet, new_witness: VKeyWitness) {
    match witness_set.vkeywitness.take() {
        Some(existing) => {
            let mut witnesses: Vec<VKeyWitness> = existing.to_vec();
            witnesses.push(new_witness);
            witness_set.vkeywitness = NonEmptySet::try_from(witnesses).ok();
        }
        None => {
            witness_set.vkeywitness = NonEmptySet::try_from(vec![new_witness]).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    // Test signing key (deterministic, for tests only)
    const TEST_SKEY_JSON: &str = r#"{
        "type": "PaymentSigningKeyShelley_ed25519",
        "description": "Payment Signing Key",
        "cborHex": "58200000000000000000000000000000000000000000000000000000000000000001"
    }"#;

    /// Build a minimal valid Conway transaction for testing.
    fn build_test_tx() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        use pallas_codec::utils::{Nullable, Set};
        use pallas_primitives::alonzo::TransactionInput;
        use pallas_primitives::conway::{
            PostAlonzoTransactionOutput, PseudoTransactionBody, PseudoTransactionOutput, Tx, Value,
            WitnessSet,
        };

        let tx_hash_bytes: [u8; 32] = [0xaa; 32];
        let input = TransactionInput {
            transaction_id: pallas_crypto::hash::Hash::from(tx_hash_bytes),
            index: 0,
        };

        let output = PseudoTransactionOutput::PostAlonzo(PostAlonzoTransactionOutput {
            address: Bytes::from(vec![0x61; 29]), // minimal shelley address
            value: Value::Coin(2_000_000),
            datum_option: None,
            script_ref: None,
        });

        let body = PseudoTransactionBody {
            inputs: Set::from(vec![input]),
            outputs: vec![output],
            fee: 200_000,
            ttl: Some(999_999),
            certificates: None,
            withdrawals: None,
            auxiliary_data_hash: None,
            validity_interval_start: None,
            mint: None,
            script_data_hash: None,
            collateral: None,
            required_signers: None,
            network_id: None,
            collateral_return: None,
            total_collateral: None,
            reference_inputs: None,
            voting_procedures: None,
            proposal_procedures: None,
            treasury_value: None,
            donation: None,
        };

        let witness_set = WitnessSet {
            vkeywitness: None,
            native_script: None,
            bootstrap_witness: None,
            plutus_v1_script: None,
            plutus_data: None,
            redeemer: None,
            plutus_v2_script: None,
            plutus_v3_script: None,
        };

        let tx = Tx {
            transaction_body: body,
            transaction_witness_set: witness_set,
            success: true,
            auxiliary_data: Nullable::Null,
        };

        let bytes = pallas_codec::minicbor::to_vec(&tx)?;
        Ok(bytes)
    }

    #[test]
    fn test_parse_skey_json_valid() -> TestResult {
        let result = parse_skey_json(TEST_SKEY_JSON, "test.skey")?;
        let vk = result.verifying_key;
        assert_eq!(vk.to_bytes().len(), 32);
        Ok(())
    }

    #[test]
    fn test_parse_skey_json_missing_cbor_hex() {
        let json = r#"{"type": "test", "description": "test"}"#;
        let result = parse_skey_json(json, "bad.skey");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("cborHex"));
    }

    #[test]
    fn test_parse_skey_json_short_cbor_hex() {
        let json = r#"{"type": "test", "cborHex": "5820aabb"}"#;
        let result = parse_skey_json(json, "short.skey");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("too short"));
    }

    #[test]
    fn test_parse_skey_json_wrong_prefix() {
        let json = r#"{"type": "test", "cborHex": "FFFF0000000000000000000000000000000000000000000000000000000000000001"}"#;
        let result = parse_skey_json(json, "bad_prefix.skey");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("5820"));
    }

    #[test]
    fn test_parse_skey_json_invalid_json() {
        let result = parse_skey_json("not json", "bad.skey");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid JSON"));
    }

    #[test]
    fn test_parse_skey_file_not_found() {
        let result = parse_skey_file("/nonexistent/path/me.skey");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("failed to read"));
    }

    #[test]
    fn test_compute_tx_hash_invalid_cbor() {
        let result = compute_tx_hash(&[0xFF, 0xFF]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("failed to decode"));
    }

    #[test]
    fn test_sign_transaction_invalid_cbor() {
        let result = sign_transaction(&[0xFF, 0xFF], "test.skey");
        assert!(result.is_err());
    }

    #[test]
    fn test_compute_tx_hash_deterministic() -> TestResult {
        let tx_bytes = build_test_tx()?;
        let hash1 = compute_tx_hash(&tx_bytes)?;
        let hash2 = compute_tx_hash(&tx_bytes)?;
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, [0u8; 32]); // should not be all zeros
        Ok(())
    }

    #[test]
    fn test_sign_transaction_roundtrip() -> TestResult {
        let tx_bytes = build_test_tx()?;

        // Parse signing key
        let parsed_key = parse_skey_json(TEST_SKEY_JSON, "test.skey")?;

        // Sign
        let signed_bytes = sign_transaction_with_key(&tx_bytes, &parsed_key)?;

        // Decode signed tx and verify witness was added
        let signed_tx: Tx = pallas_codec::minicbor::decode(&signed_bytes)?;
        let witnesses = signed_tx
            .transaction_witness_set
            .vkeywitness
            .ok_or("should have vkeywitness")?;
        let witness_vec: Vec<VKeyWitness> = witnesses.to_vec();
        assert_eq!(witness_vec.len(), 1);

        let vkey_bytes: &[u8] = &witness_vec[0].vkey;
        let sig_bytes: &[u8] = &witness_vec[0].signature;
        assert_eq!(vkey_bytes.len(), 32);
        assert_eq!(sig_bytes.len(), 64);

        // Verify the signature is valid
        let vk_arr: [u8; 32] = vkey_bytes.try_into()?;
        let sig_arr: [u8; 64] = sig_bytes.try_into()?;
        let vk = ed25519_dalek::VerifyingKey::from_bytes(&vk_arr)?;
        let sig = ed25519_dalek::Signature::from_bytes(&sig_arr);

        let tx_hash = compute_tx_hash(&tx_bytes)?;
        vk.verify_strict(&tx_hash, &sig)?;

        Ok(())
    }

    #[test]
    fn test_vkey_witness_construction() -> TestResult {
        let parsed_key = parse_skey_json(TEST_SKEY_JSON, "test.skey")?;

        let msg = [0u8; 32];
        let signature = parsed_key.signing_key.sign(&msg);

        let witness = VKeyWitness {
            vkey: Bytes::from(parsed_key.verifying_key.to_bytes().to_vec()),
            signature: Bytes::from(signature.to_bytes().to_vec()),
        };

        let vkey_bytes: &[u8] = &witness.vkey;
        let sig_bytes: &[u8] = &witness.signature;
        assert_eq!(vkey_bytes.len(), 32);
        assert_eq!(sig_bytes.len(), 64);
        Ok(())
    }
}
