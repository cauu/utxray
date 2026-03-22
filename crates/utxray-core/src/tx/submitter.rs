//! Transaction submission: decode tx, detect network, submit via Blockfrost.

use serde::Serialize;

use crate::backend::blockfrost::BlockfrostBackend;
use crate::output::Output;

use super::signer;

/// Errors specific to transaction submission.
#[derive(Debug, thiserror::Error)]
pub enum SubmitError {
    #[error("invalid transaction CBOR hex: {0}")]
    InvalidHex(String),

    #[error("failed to decode transaction: {0}")]
    DecodeFailed(String),

    #[error("mainnet safety block: refusing to submit to mainnet without --allow-mainnet")]
    MainnetSafetyBlock,

    #[error("backend error: {0}")]
    BackendError(String),
}

/// Output data for a successful submission.
#[derive(Debug, Serialize)]
pub struct SubmitOutput {
    pub tx_hash: String,
    pub network: String,
    pub hint: String,
}

/// Output data for submission errors that should be returned as structured JSON
/// (not as Rust errors).
#[derive(Debug, Serialize)]
pub struct SubmitErrorOutput {
    pub error_code: String,
    pub severity: String,
    pub message: String,
}

/// Submit a signed transaction to the network via Blockfrost.
///
/// Returns `Output<SubmitOutput>` on success. Mainnet safety check is
/// enforced unless `allow_mainnet` is true.
pub async fn submit_transaction(
    tx_cbor_hex: &str,
    network: &str,
    allow_mainnet: bool,
    backend: &BlockfrostBackend,
) -> Result<Output<SubmitOutput>, SubmitError> {
    // 1. Mainnet safety check (before any other work)
    if network == "mainnet" && !allow_mainnet {
        return Err(SubmitError::MainnetSafetyBlock);
    }

    // 2. Validate hex
    let tx_bytes = hex::decode(tx_cbor_hex)
        .map_err(|e| SubmitError::InvalidHex(format!("invalid hex: {e}")))?;

    // 3. Compute tx hash from the body
    let tx_hash_bytes =
        signer::compute_tx_hash(&tx_bytes).map_err(|e| SubmitError::DecodeFailed(e.to_string()))?;
    let tx_hash = hex::encode(tx_hash_bytes);

    // 4. Submit via backend
    let _submitted_hash = backend
        .submit_tx(tx_cbor_hex)
        .await
        .map_err(|e| SubmitError::BackendError(e.to_string()))?;

    Ok(Output::ok(SubmitOutput {
        tx_hash,
        network: network.to_string(),
        hint: "Transaction submitted to mempool. Wait ~20s before querying updated UTXOs."
            .to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[tokio::test]
    async fn test_mainnet_safety_block() -> TestResult {
        let backend = BlockfrostBackend::new("fake_project_id", "mainnet")?;
        let result = submit_transaction("aabb", "mainnet", false, &backend).await;
        assert!(result.is_err());
        let err = result.err().ok_or("expected error")?;
        assert!(matches!(err, SubmitError::MainnetSafetyBlock));
        assert!(err.to_string().contains("mainnet"));
        Ok(())
    }

    #[tokio::test]
    async fn test_invalid_hex() -> TestResult {
        let backend = BlockfrostBackend::new("fake_project_id", "preview")?;
        let result = submit_transaction("not_hex!", "preview", false, &backend).await;
        assert!(result.is_err());
        let err = result.err().ok_or("expected error")?;
        assert!(matches!(err, SubmitError::InvalidHex(_)));
        Ok(())
    }
}
