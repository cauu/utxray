use serde::Serialize;
use std::collections::HashMap;

use crate::backend::blockfrost::BlockfrostBackend;
use crate::backend::UtxoInfo;
use crate::error::Severity;
use crate::output::Output;

/// Error types for UTxO diff operations.
#[derive(Debug, thiserror::Error)]
pub enum UtxoDiffError {
    #[error("missing required argument: {0}")]
    MissingArgument(String),

    #[error("backend error: {0}")]
    Backend(String),

    #[error("invalid transaction hash: {0}")]
    InvalidTxHash(String),
}

/// A single UTxO change entry.
#[derive(Debug, Serialize)]
pub struct UtxoChange {
    pub tx_hash: String,
    pub index: u32,
    pub change_type: String,
    pub lovelace: u64,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub tokens: HashMap<String, HashMap<String, u64>>,
}

/// The output data for utxo diff.
#[derive(Debug, Serialize)]
pub struct UtxoDiffOutput {
    pub address: String,
    pub mode: String,
    pub added: Vec<UtxoChange>,
    pub removed: Vec<UtxoChange>,
    pub current_utxo_count: usize,
    pub current_lovelace_total: u64,
}

/// Error output data for diff failures.
#[derive(Debug, Serialize)]
pub struct UtxoDiffErrorData {
    pub error: String,
}

/// Compute UTxO diff for an address between two transactions.
///
/// In v1, Blockfrost does not support historical UTxO queries.
/// We query the current UTxO set and check which UTxOs were created/consumed
/// by the specified transactions.
pub async fn diff_by_tx(
    address: &str,
    before_tx: &str,
    after_tx: &str,
    backend: &BlockfrostBackend,
) -> Result<Output<UtxoDiffOutput>, UtxoDiffError> {
    // Query current UTxOs at the address
    let current_utxos = backend
        .query_utxos(address)
        .await
        .map_err(|e| UtxoDiffError::Backend(e.to_string()))?;

    let current_lovelace_total: u64 = current_utxos.iter().map(|u| u.value.lovelace).sum();
    let current_utxo_count = current_utxos.len();

    // Categorize UTxOs by their relationship to the specified transactions
    let mut added = Vec::new();
    let mut removed = Vec::new();

    for utxo in &current_utxos {
        // UTxOs created by the after_tx are "added"
        if utxo.tx_hash == after_tx {
            added.push(utxo_to_change(utxo, "added"));
        }
    }

    // UTxOs from the before_tx that are NOT in the current set were "removed" (spent)
    // Since they are no longer in the UTxO set, we can't see them directly.
    // We note this as a limitation: if before_tx outputs were spent, they won't appear
    // in current UTxOs. We flag any current UTxOs from before_tx as "retained".
    let before_tx_utxos: Vec<_> = current_utxos
        .iter()
        .filter(|u| u.tx_hash == before_tx)
        .collect();

    // If no UTxOs from before_tx remain, they were likely consumed
    if before_tx_utxos.is_empty() && !before_tx.is_empty() {
        // We can infer that UTxOs from before_tx at this address were consumed
        // but we don't know their exact values without querying the tx details
        removed.push(UtxoChange {
            tx_hash: before_tx.to_string(),
            index: 0,
            change_type: "inferred_removed".to_string(),
            lovelace: 0,
            tokens: HashMap::new(),
        });
    }

    let output = UtxoDiffOutput {
        address: address.to_string(),
        mode: "by_tx".to_string(),
        added,
        removed,
        current_utxo_count,
        current_lovelace_total,
    };

    let mut result = Output::ok(output);
    result = result.with_warning(
        Severity::Info,
        "v1 limitation: historical UTxO queries not supported by Blockfrost. \
         Diff is computed against current UTxO set.",
    );

    Ok(result)
}

/// Compute UTxO diff for an address between two slots.
///
/// In v1, this has the same Blockfrost limitation as by_tx mode.
/// We query current UTxOs and note the limitation.
pub async fn diff_by_slot(
    address: &str,
    _before_slot: u64,
    _after_slot: u64,
    backend: &BlockfrostBackend,
) -> Result<Output<UtxoDiffOutput>, UtxoDiffError> {
    let current_utxos = backend
        .query_utxos(address)
        .await
        .map_err(|e| UtxoDiffError::Backend(e.to_string()))?;

    let current_lovelace_total: u64 = current_utxos.iter().map(|u| u.value.lovelace).sum();
    let current_utxo_count = current_utxos.len();

    let output = UtxoDiffOutput {
        address: address.to_string(),
        mode: "by_slot".to_string(),
        added: Vec::new(),
        removed: Vec::new(),
        current_utxo_count,
        current_lovelace_total,
    };

    let mut result = Output::ok(output);
    result = result.with_warning(
        Severity::Warning,
        "v1 limitation: slot-based historical UTxO diff requires an indexer with \
         point-in-time queries. Returning current UTxO set only.",
    );

    Ok(result)
}

fn utxo_to_change(utxo: &UtxoInfo, change_type: &str) -> UtxoChange {
    UtxoChange {
        tx_hash: utxo.tx_hash.clone(),
        index: utxo.index,
        change_type: change_type.to_string(),
        lovelace: utxo.value.lovelace,
        tokens: utxo.value.tokens.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utxo_diff_error_display() {
        let err = UtxoDiffError::MissingArgument("--address".to_string());
        assert_eq!(err.to_string(), "missing required argument: --address");

        let err = UtxoDiffError::Backend("connection failed".to_string());
        assert_eq!(err.to_string(), "backend error: connection failed");

        let err = UtxoDiffError::InvalidTxHash("abc".to_string());
        assert_eq!(err.to_string(), "invalid transaction hash: abc");
    }

    #[test]
    fn test_utxo_change_serialize() -> Result<(), Box<dyn std::error::Error>> {
        let change = UtxoChange {
            tx_hash: "abc123".to_string(),
            index: 0,
            change_type: "added".to_string(),
            lovelace: 5_000_000,
            tokens: HashMap::new(),
        };
        let json = serde_json::to_value(&change)?;
        assert_eq!(json["tx_hash"], "abc123");
        assert_eq!(json["lovelace"], 5_000_000);
        assert_eq!(json["change_type"], "added");
        // Empty tokens should be skipped
        assert!(json.get("tokens").is_none());
        Ok(())
    }

    #[test]
    fn test_utxo_diff_output_serialize() -> Result<(), Box<dyn std::error::Error>> {
        let output = UtxoDiffOutput {
            address: "addr_test1".to_string(),
            mode: "by_tx".to_string(),
            added: vec![],
            removed: vec![],
            current_utxo_count: 3,
            current_lovelace_total: 10_000_000,
        };
        let json = serde_json::to_value(&output)?;
        assert_eq!(json["address"], "addr_test1");
        assert_eq!(json["mode"], "by_tx");
        assert_eq!(json["current_utxo_count"], 3);
        assert_eq!(json["current_lovelace_total"], 10_000_000);
        Ok(())
    }
}
