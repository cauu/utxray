pub mod blockfrost;

use serde::Serialize;
use std::collections::HashMap;

/// Information about a single UTxO
#[derive(Debug, Clone, Serialize)]
pub struct UtxoInfo {
    pub tx_hash: String,
    pub index: u32,
    pub value: UtxoValue,
    pub address: String,
    pub datum_hash: Option<String>,
    pub inline_datum: Option<serde_json::Value>,
    pub reference_script_hash: Option<String>,
}

/// Value contained in a UTxO
#[derive(Debug, Clone, Serialize)]
pub struct UtxoValue {
    pub lovelace: u64,
    pub tokens: HashMap<String, HashMap<String, u64>>,
}

/// Resolved datum information
#[derive(Debug, Clone, Serialize)]
pub struct DatumInfo {
    pub hash: String,
    pub source: String,
    pub decoded: serde_json::Value,
}

/// Chain tip information
#[derive(Debug, Clone, Serialize)]
pub struct TipInfo {
    pub slot: u64,
    pub block_hash: String,
    pub block_height: u64,
    pub epoch: u64,
    pub time_s: u64,
}

/// Result of evaluating a transaction
#[derive(Debug, Clone, Serialize)]
pub struct EvaluationResult {
    pub redeemers: Vec<EvaluatedRedeemer>,
}

/// A single evaluated redeemer with execution units
#[derive(Debug, Clone, Serialize)]
pub struct EvaluatedRedeemer {
    pub tag: String,
    pub index: u32,
    pub exec_units: ExUnits,
}

/// Execution units (CPU and memory)
#[derive(Debug, Clone, Serialize)]
pub struct ExUnits {
    pub cpu: u64,
    pub mem: u64,
}
