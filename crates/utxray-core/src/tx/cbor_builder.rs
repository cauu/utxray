//! Constructs real Cardano Conway-era transaction CBOR from a TxSpec.
//!
//! Uses pallas-primitives Conway types for encoding, with pallas-addresses
//! for bech32 address decoding.

use pallas_addresses::Address;
use pallas_codec::utils::{Bytes, NonEmptyKeyValuePairs, NonEmptySet, Nullable, PositiveCoin, Set};
use pallas_crypto::hash::Hash;
use pallas_primitives::alonzo::TransactionInput;
use pallas_primitives::conway::{
    ExUnits, NetworkId, PostAlonzoTransactionOutput, PseudoTransactionBody,
    PseudoTransactionOutput, PseudoTx, RedeemerTag, Redeemers, RedeemersKey, RedeemersValue,
    TransactionBody, Tx, Value, WitnessSet,
};

use crate::cbor::encode::json_to_plutus_data;
use crate::cbor::script_data_hash::compute_hash_from_parts;
use crate::tx::builder::{
    total_input_lovelace, total_output_lovelace, MintEntry, TxBuildError, TxSpec,
};
use std::collections::BTreeMap;

/// Protocol parameters for fee estimation (mainnet defaults).
const MIN_FEE_COEFFICIENT: u64 = 44; // lovelace per byte
const MIN_FEE_CONSTANT: u64 = 155_381;

/// Default execution units for placeholders when not specified.
const DEFAULT_EX_UNITS_MEM: u64 = 500_000;
const DEFAULT_EX_UNITS_STEPS: u64 = 200_000_000;

/// Minimum lovelace per output (simplified check: 1 ADA).
const MIN_UTXO_LOVELACE: u64 = 1_000_000;

/// Parse a UTxO reference string "txhash#index" into TransactionInput.
fn parse_utxo_ref(utxo: &str) -> Result<TransactionInput, TxBuildError> {
    let parts: Vec<&str> = utxo.splitn(2, '#').collect();
    if parts.len() != 2 {
        return Err(TxBuildError::InvalidSpec(format!(
            "invalid UTxO reference format (expected 'txhash#index'): {utxo}"
        )));
    }

    let tx_hash_hex = parts[0];
    let index_str = parts[1];

    // Validate the tx hash - must be exactly 32 bytes (64 hex chars)
    if tx_hash_hex.len() != 64 {
        return Err(TxBuildError::InvalidSpec(format!(
            "tx hash must be exactly 64 hex chars (32 bytes), got {} chars in UTxO ref '{utxo}'",
            tx_hash_hex.len()
        )));
    }

    let hash_bytes = hex::decode(tx_hash_hex).map_err(|e| {
        TxBuildError::InvalidSpec(format!("invalid tx hash hex in UTxO ref '{utxo}': {e}"))
    })?;

    let hash: Hash<32> = Hash::from(
        <[u8; 32]>::try_from(hash_bytes.as_slice())
            .map_err(|_| TxBuildError::InvalidSpec(format!("tx hash must be 32 bytes: {utxo}")))?,
    );

    let index: u64 = index_str
        .parse()
        .map_err(|e| TxBuildError::InvalidSpec(format!("invalid UTxO index in '{utxo}': {e}")))?;

    Ok(TransactionInput {
        transaction_id: hash,
        index,
    })
}

/// Decode a bech32 Cardano address into raw bytes.
/// Only accepts valid bech32 (addr_test1..., addr1...) or valid hex-encoded addresses.
fn decode_address(addr_str: &str) -> Result<Bytes, TxBuildError> {
    // Try bech32 first
    if let Ok(addr) = Address::from_bech32(addr_str) {
        return Ok(Bytes::from(addr.to_vec()));
    }

    // Try hex decoding
    if let Ok(bytes) = hex::decode(addr_str) {
        if !bytes.is_empty() {
            return Ok(Bytes::from(bytes));
        }
    }

    Err(TxBuildError::InvalidSpec(format!(
        "invalid address '{addr_str}': must be a valid bech32 address (addr_test1.../addr1...) or hex-encoded address"
    )))
}

/// Parse a hex-encoded key hash (28 bytes) for required signers.
fn parse_keyhash_28(hex_str: &str) -> Result<Hash<28>, TxBuildError> {
    if hex_str.len() != 56 {
        return Err(TxBuildError::InvalidSpec(format!(
            "signer hash must be exactly 56 hex chars (28 bytes), got {} chars: '{hex_str}'",
            hex_str.len()
        )));
    }

    let bytes = hex::decode(hex_str)
        .map_err(|e| TxBuildError::InvalidSpec(format!("invalid key hash hex '{hex_str}': {e}")))?;

    Ok(Hash::from(<[u8; 28]>::try_from(bytes.as_slice()).map_err(
        |_| TxBuildError::InvalidSpec(format!("key hash must be 28 bytes: {hex_str}")),
    )?))
}

/// A map of policy_id (hex) -> asset_name -> amount for tracking multi-asset values.
type TokenMap = BTreeMap<String, BTreeMap<String, u64>>;

/// Sum tokens from all inputs that have value.tokens specified.
fn sum_input_tokens(spec: &TxSpec) -> TokenMap {
    let mut tokens: TokenMap = BTreeMap::new();
    for input in &spec.inputs {
        if let Some(ref val) = input.value {
            if let Some(ref toks) = val.tokens {
                merge_tokens(&mut tokens, toks);
            }
        }
    }
    for si in &spec.script_inputs {
        if let Some(ref val) = si.value {
            if let Some(ref toks) = val.tokens {
                merge_tokens(&mut tokens, toks);
            }
        }
    }
    tokens
}

/// Sum tokens from all outputs.
fn sum_output_tokens(spec: &TxSpec) -> TokenMap {
    let mut tokens: TokenMap = BTreeMap::new();
    for output in &spec.outputs {
        if let Some(ref toks) = output.value.tokens {
            merge_tokens(&mut tokens, toks);
        }
    }
    tokens
}

/// Merge source tokens into dest (additive).
fn merge_tokens(
    dest: &mut TokenMap,
    source: &std::collections::HashMap<String, std::collections::HashMap<String, u64>>,
) {
    for (policy, assets) in source {
        let policy_entry = dest.entry(policy.clone()).or_default();
        for (asset_name, &amount) in assets {
            *policy_entry.entry(asset_name.clone()).or_insert(0) += amount;
        }
    }
}

/// Compute remaining tokens: input_tokens - output_tokens.
/// Returns only entries with amount > 0.
fn remaining_tokens(input_tokens: &TokenMap, output_tokens: &TokenMap) -> TokenMap {
    let mut remaining: TokenMap = input_tokens.clone();
    for (policy, assets) in output_tokens {
        if let Some(policy_entry) = remaining.get_mut(policy) {
            for (asset_name, &amount) in assets {
                if let Some(entry) = policy_entry.get_mut(asset_name) {
                    *entry = entry.saturating_sub(amount);
                }
            }
        }
    }
    // Remove zero entries
    remaining.retain(|_, assets| {
        assets.retain(|_, amount| *amount > 0);
        !assets.is_empty()
    });
    remaining
}

/// Validate that a policy ID is exactly 56 hex chars (28 bytes).
fn validate_policy_id(policy_hex: &str) -> Result<(), TxBuildError> {
    if policy_hex.len() != 56 {
        return Err(TxBuildError::InvalidSpec(format!(
            "policy_id must be exactly 56 hex chars (28 bytes), got {} chars: '{policy_hex}'",
            policy_hex.len()
        )));
    }
    hex::decode(policy_hex).map_err(|e| {
        TxBuildError::InvalidSpec(format!("invalid policy_id hex '{policy_hex}': {e}"))
    })?;
    Ok(())
}

/// Validate that an asset name is valid hex (even length, valid hex chars).
/// Empty string is allowed (represents the empty asset name).
fn validate_asset_name_hex(name: &str) -> Result<Vec<u8>, TxBuildError> {
    if name.is_empty() {
        return Ok(Vec::new());
    }
    hex::decode(name).map_err(|e| {
        TxBuildError::InvalidSpec(format!(
            "asset_name must be valid hex-encoded bytes, got '{name}': {e}"
        ))
    })
}

/// Build a pallas Multiasset value from a TokenMap.
fn build_multiasset_value(lovelace: u64, tokens: &TokenMap) -> Result<Value, TxBuildError> {
    if tokens.is_empty() {
        return Ok(Value::Coin(lovelace));
    }

    let mut outer_pairs = Vec::new();
    for (policy_hex, assets) in tokens {
        validate_policy_id(policy_hex)?;

        let policy_bytes = hex::decode(policy_hex).map_err(|e| {
            TxBuildError::InvalidSpec(format!("invalid policy ID hex '{policy_hex}': {e}"))
        })?;

        let pid: Hash<28> =
            Hash::from(<[u8; 28]>::try_from(policy_bytes.as_slice()).map_err(|_| {
                TxBuildError::InvalidSpec("policy ID must be 28 bytes".to_string())
            })?);

        let mut inner_pairs = Vec::new();
        for (name, &amount) in assets {
            let name_bytes = validate_asset_name_hex(name)?;
            if let Ok(pc) = PositiveCoin::try_from(amount) {
                inner_pairs.push((Bytes::from(name_bytes), pc));
            }
        }

        if let Ok(inner) = NonEmptyKeyValuePairs::try_from(inner_pairs) {
            outer_pairs.push((pid, inner));
        }
    }

    if outer_pairs.is_empty() {
        return Ok(Value::Coin(lovelace));
    }

    let multiasset = NonEmptyKeyValuePairs::try_from(outer_pairs)
        .map_err(|_| TxBuildError::InvalidSpec("failed to create multiasset".to_string()))?;
    Ok(Value::Multiasset(lovelace, multiasset))
}

/// Build a Conway-era transaction output.
fn build_tx_output(
    address: &str,
    lovelace: u64,
    _datum: &Option<serde_json::Value>,
) -> Result<PseudoTransactionOutput<PostAlonzoTransactionOutput>, TxBuildError> {
    let addr_bytes = decode_address(address)?;

    let value = Value::Coin(lovelace);

    Ok(PseudoTransactionOutput::PostAlonzo(
        PostAlonzoTransactionOutput {
            address: addr_bytes,
            value,
            datum_option: None,
            script_ref: None,
        },
    ))
}

/// Build a Conway-era transaction output with optional multi-asset tokens.
fn build_tx_output_with_tokens(
    address: &str,
    lovelace: u64,
    tokens: &TokenMap,
) -> Result<PseudoTransactionOutput<PostAlonzoTransactionOutput>, TxBuildError> {
    let addr_bytes = decode_address(address)?;
    let value = build_multiasset_value(lovelace, tokens)?;

    Ok(PseudoTransactionOutput::PostAlonzo(
        PostAlonzoTransactionOutput {
            address: addr_bytes,
            value,
            datum_option: None,
            script_ref: None,
        },
    ))
}

/// Map a network name string to a Cardano NetworkId.
/// "mainnet" maps to NetworkId::Two, everything else to NetworkId::One.
fn network_id_from_str(network: &str) -> NetworkId {
    if network == "mainnet" {
        NetworkId::Two
    } else {
        NetworkId::One
    }
}

/// Build the Conway transaction body from a TxSpec.
///
/// `change_lovelace`: if `Some(amount)`, a change output is appended to `change_address`.
/// `change_tokens`: if non-empty, the change output includes these tokens.
/// `script_data_hash`: if `Some(hash)`, it is set on the transaction body.
/// `network`: the target network name (e.g., "mainnet", "preview", "preprod").
fn build_transaction_body(
    spec: &TxSpec,
    fee: u64,
    change_lovelace: Option<u64>,
    change_tokens: &TokenMap,
    script_data_hash: Option<Hash<32>>,
    network: &str,
) -> Result<TransactionBody, TxBuildError> {
    // Collect all inputs (pubkey + script)
    let mut inputs = Vec::new();
    for input in &spec.inputs {
        inputs.push(parse_utxo_ref(&input.utxo)?);
    }
    for si in &spec.script_inputs {
        inputs.push(parse_utxo_ref(&si.utxo)?);
    }
    // Sort inputs by (tx_id, index) as required by the Cardano ledger
    inputs.sort_by(|a, b| {
        a.transaction_id
            .as_ref()
            .cmp(b.transaction_id.as_ref())
            .then(a.index.cmp(&b.index))
    });

    // Build outputs
    let mut outputs = Vec::new();
    for output in &spec.outputs {
        outputs.push(build_tx_output(
            &output.address,
            output.value.lovelace,
            &output.datum,
        )?);
    }

    // Append change output if we have computed change (with any remaining tokens)
    if let Some(change_ada) = change_lovelace {
        if change_ada > 0 || !change_tokens.is_empty() {
            let lovelace = if change_ada > 0 {
                change_ada
            } else {
                MIN_UTXO_LOVELACE
            };
            outputs.push(build_tx_output_with_tokens(
                &spec.change_address,
                lovelace,
                change_tokens,
            )?);
        }
    } else if !change_tokens.is_empty() {
        // Tokens remaining but no lovelace change computed; include with min-UTxO
        outputs.push(build_tx_output_with_tokens(
            &spec.change_address,
            MIN_UTXO_LOVELACE,
            change_tokens,
        )?);
    }

    // TTL (validity.to_slot)
    let ttl = spec.validity.as_ref().and_then(|v| v.to_slot);

    // Validity interval start
    let validity_interval_start = spec.validity.as_ref().and_then(|v| v.from_slot);

    // Mint
    let mint = build_mint(spec)?;

    // Collateral
    let collateral = if let Some(ref coll_str) = spec.collateral {
        let coll_input = parse_utxo_ref(coll_str)?;
        Some(NonEmptySet::try_from(vec![coll_input]).map_err(|_| {
            TxBuildError::InvalidSpec("failed to create collateral set".to_string())
        })?)
    } else {
        None
    };

    // Required signers
    let required_signers = if spec.required_signers.is_empty() {
        None
    } else {
        let mut signers = Vec::new();
        for s in &spec.required_signers {
            signers.push(parse_keyhash_28(s)?);
        }
        Some(NonEmptySet::try_from(signers).map_err(|_| {
            TxBuildError::InvalidSpec("failed to create required_signers set".to_string())
        })?)
    };

    // Reference inputs
    let reference_inputs = if spec.reference_inputs.is_empty() {
        None
    } else {
        // Reference inputs are just JSON values; try to parse them as UTxO refs
        let mut refs = Vec::new();
        for ri in &spec.reference_inputs {
            if let Some(utxo_str) = ri.as_str() {
                refs.push(parse_utxo_ref(utxo_str)?);
            }
        }
        if refs.is_empty() {
            None
        } else {
            Some(NonEmptySet::try_from(refs).map_err(|_| {
                TxBuildError::InvalidSpec("failed to create reference_inputs set".to_string())
            })?)
        }
    };

    Ok(PseudoTransactionBody {
        inputs: Set::from(inputs),
        outputs,
        fee,
        ttl,
        certificates: None,
        withdrawals: None,
        auxiliary_data_hash: None,
        validity_interval_start,
        mint,
        script_data_hash,
        collateral,
        required_signers,
        network_id: Some(network_id_from_str(network)),
        collateral_return: None,
        total_collateral: None,
        reference_inputs,
        voting_procedures: None,
        proposal_procedures: None,
        treasury_value: None,
        donation: None,
    })
}

/// Build mint multiasset from the spec.
fn build_mint(spec: &TxSpec) -> Result<Option<pallas_primitives::conway::Mint>, TxBuildError> {
    let mint_map = match spec.mint {
        Some(ref m) if !m.is_empty() => m,
        _ => return Ok(None),
    };

    // Collect policy_id -> [(asset_name, amount)] in sorted order
    let mut policy_entries: BTreeMap<Vec<u8>, Vec<(Vec<u8>, i64)>> = BTreeMap::new();

    for (policy_hex, entry) in mint_map {
        validate_policy_id(policy_hex)?;

        let policy_bytes = hex::decode(policy_hex).map_err(|e| {
            TxBuildError::InvalidSpec(format!("invalid policy ID hex '{policy_hex}': {e}"))
        })?;

        let mut assets: Vec<(Vec<u8>, i64)> = Vec::new();
        for (name, &amount) in &entry.assets {
            let name_bytes = validate_asset_name_hex(name)?;
            assets.push((name_bytes, amount as i64));
        }

        policy_entries
            .entry(policy_bytes)
            .or_default()
            .extend(assets);
    }

    let mut outer_pairs = Vec::new();
    for (policy_id, assets) in policy_entries {
        let pid: Hash<28> =
            Hash::from(<[u8; 28]>::try_from(policy_id.as_slice()).map_err(|_| {
                TxBuildError::InvalidSpec("policy ID must be 28 bytes".to_string())
            })?);

        let inner_pairs: Vec<(Bytes, pallas_codec::utils::NonZeroInt)> = assets
            .into_iter()
            .filter_map(|(name, amount)| {
                pallas_codec::utils::NonZeroInt::try_from(amount)
                    .ok()
                    .map(|nz| (Bytes::from(name), nz))
            })
            .collect();

        if let Ok(inner) = NonEmptyKeyValuePairs::try_from(inner_pairs) {
            outer_pairs.push((pid, inner));
        }
    }

    if outer_pairs.is_empty() {
        return Ok(None);
    }

    NonEmptyKeyValuePairs::try_from(outer_pairs)
        .map(Some)
        .map_err(|_| TxBuildError::InvalidSpec("failed to create mint multiasset".to_string()))
}

/// Build the witness set with redeemers and plutus data.
///
/// `exec_units_map`: optional map from (tag, index) to (cpu, mem) for real ExUnits.
fn build_witness_set(
    spec: &TxSpec,
    exec_units_map: Option<&ExecUnitsMap>,
) -> Result<WitnessSet, TxBuildError> {
    let mut redeemer_entries = Vec::new();
    let mut plutus_data_list = Vec::new();

    // Build the sorted list of ALL inputs for correct redeemer indexing.
    // Redeemer index for spend must be the position of the script input
    // in the sorted (all inputs) list.
    let mut all_inputs: Vec<(Vec<u8>, u64)> = Vec::new();
    for input in &spec.inputs {
        let parsed = parse_utxo_ref(&input.utxo)?;
        all_inputs.push((parsed.transaction_id.as_ref().to_vec(), parsed.index));
    }
    for si in &spec.script_inputs {
        let parsed = parse_utxo_ref(&si.utxo)?;
        all_inputs.push((parsed.transaction_id.as_ref().to_vec(), parsed.index));
    }
    all_inputs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    // Redeemers from script inputs
    for (i, si) in spec.script_inputs.iter().enumerate() {
        let redeemer_data = json_to_plutus_data(&si.redeemer).map_err(|e| {
            TxBuildError::InvalidSpec(format!("invalid redeemer for script_input[{i}]: {e}"))
        })?;

        let tag = match si.purpose.as_str() {
            "spend" => RedeemerTag::Spend,
            "mint" => RedeemerTag::Mint,
            "withdrawal" | "withdraw" => RedeemerTag::Reward,
            "certificate" | "cert" => RedeemerTag::Cert,
            "vote" => RedeemerTag::Vote,
            "propose" => RedeemerTag::Propose,
            _ => {
                return Err(TxBuildError::InvalidSpec(format!(
                    "unknown purpose '{}'; expected one of: spend, mint, withdrawal, certificate, propose, vote (aliases: withdraw, cert)",
                    si.purpose
                )));
            }
        };

        // For spend redeemers, the index is the position in the sorted inputs list
        let redeemer_index = if tag == RedeemerTag::Spend {
            let parsed = parse_utxo_ref(&si.utxo)?;
            let needle = (parsed.transaction_id.as_ref().to_vec(), parsed.index);
            all_inputs
                .iter()
                .position(|x| x == &needle)
                .ok_or_else(|| {
                    TxBuildError::InvalidSpec(format!(
                        "script_input[{i}] utxo not found in sorted inputs list"
                    ))
                })? as u32
        } else {
            i as u32
        };

        // Look up exec units from the map, or use defaults
        let ex_units = exec_units_map
            .and_then(|m| m.get(&tag, redeemer_index))
            .unwrap_or(ExUnits {
                mem: DEFAULT_EX_UNITS_MEM,
                steps: DEFAULT_EX_UNITS_STEPS,
            });

        redeemer_entries.push((
            RedeemersKey {
                tag,
                index: redeemer_index,
            },
            RedeemersValue {
                data: redeemer_data,
                ex_units,
            },
        ));

        // If datum is provided in Plutus JSON schema format (has constructor/int/bytes/list/map),
        // include it in the witness set plutus data
        if si.datum.is_object() && has_plutus_data_shape(&si.datum) {
            if let Ok(datum_data) = json_to_plutus_data(&si.datum) {
                plutus_data_list.push(datum_data);
            }
        }
    }

    // Redeemers from mint entries
    if let Some(ref mint_map) = spec.mint {
        // Sort by policy_id for deterministic ordering
        let mut sorted_mints: Vec<(&String, &MintEntry)> = mint_map.iter().collect();
        sorted_mints.sort_by_key(|(k, _)| k.as_str());

        for (i, (_policy_id, entry)) in sorted_mints.into_iter().enumerate() {
            let redeemer_data = json_to_plutus_data(&entry.redeemer).map_err(|e| {
                TxBuildError::InvalidSpec(format!("invalid redeemer for mint policy: {e}"))
            })?;

            let mint_index = i as u32;
            let ex_units = exec_units_map
                .and_then(|m| m.get(&RedeemerTag::Mint, mint_index))
                .unwrap_or(ExUnits {
                    mem: DEFAULT_EX_UNITS_MEM,
                    steps: DEFAULT_EX_UNITS_STEPS,
                });

            redeemer_entries.push((
                RedeemersKey {
                    tag: RedeemerTag::Mint,
                    index: mint_index,
                },
                RedeemersValue {
                    data: redeemer_data,
                    ex_units,
                },
            ));
        }
    }

    let redeemer = if redeemer_entries.is_empty() {
        None
    } else {
        Some(Redeemers::from(
            NonEmptyKeyValuePairs::try_from(redeemer_entries)
                .map_err(|_| TxBuildError::InvalidSpec("failed to create redeemers".to_string()))?,
        ))
    };

    let plutus_data = if plutus_data_list.is_empty() {
        None
    } else {
        Some(NonEmptySet::try_from(plutus_data_list).map_err(|_| {
            TxBuildError::InvalidSpec("failed to create plutus_data set".to_string())
        })?)
    };

    Ok(WitnessSet {
        vkeywitness: None,
        native_script: None,
        bootstrap_witness: None,
        plutus_v1_script: None,
        plutus_data,
        redeemer,
        plutus_v2_script: None,
        plutus_v3_script: None,
    })
}

/// Check if a JSON value looks like a Plutus data schema
/// (has "constructor", "int", "bytes", "list", or "map" at top level).
fn has_plutus_data_shape(value: &serde_json::Value) -> bool {
    if let Some(obj) = value.as_object() {
        obj.contains_key("constructor")
            || obj.contains_key("int")
            || obj.contains_key("bytes")
            || obj.contains_key("list")
            || obj.contains_key("map")
    } else {
        false
    }
}

/// Compute the script_data_hash for the transaction when Plutus scripts are present.
///
/// Per the Cardano ledger spec, the script data hash is:
///   blake2b_256(redeemers_cbor || datums_cbor || cost_models_cbor)
///
/// Returns `(Option<Hash>, Vec<String>)` — the hash (if computable) and any warnings.
/// When cost models are not available, returns None for the hash and a warning.
fn compute_script_data_hash_for_tx(
    witness_set: &WitnessSet,
    cost_models: Option<&serde_json::Value>,
) -> Result<(Option<Hash<32>>, Vec<String>), TxBuildError> {
    let mut warnings = Vec::new();

    // Only compute when there are redeemers (i.e., Plutus scripts are used)
    let redeemers = match &witness_set.redeemer {
        Some(r) => r,
        None => return Ok((None, warnings)),
    };

    // If there are Plutus scripts but no cost model, skip script_data_hash
    let cost_models_json = match cost_models {
        Some(cm) => cm.clone(),
        None => {
            warnings.push(
                "script_data_hash not computed: cost model not available. \
                 Run 'utxray context params' to fetch protocol parameters, \
                 or provide --exec-units with cost model data."
                    .to_string(),
            );
            return Ok((None, warnings));
        }
    };

    // Encode the redeemers to CBOR
    let redeemers_cbor = pallas_codec::minicbor::to_vec(redeemers)
        .map_err(|e| TxBuildError::WriteError(format!("CBOR redeemer encoding failed: {e}")))?;

    // Encode datums to CBOR (empty bytes if no datums, per ledger spec)
    let datums_cbor = if let Some(ref datums) = witness_set.plutus_data {
        pallas_codec::minicbor::to_vec(datums)
            .map_err(|e| TxBuildError::WriteError(format!("CBOR datum encoding failed: {e}")))?
    } else {
        Vec::new()
    };

    let cost_models_cbor = crate::cbor::script_data_hash::encode_cost_models(&cost_models_json)
        .map_err(|e| TxBuildError::WriteError(format!("cost model encoding failed: {e}")))?;

    let hash_bytes = compute_hash_from_parts(&redeemers_cbor, &datums_cbor, &cost_models_cbor);
    Ok((Some(Hash::from(hash_bytes)), warnings))
}

/// A map from (redeemer_tag, index) to ExUnits (cpu, mem).
/// Used to apply real execution units from `tx evaluate` output.
#[derive(Debug, Default)]
pub struct ExecUnitsMap {
    entries: Vec<(RedeemerTag, u32, ExUnits)>,
}

impl ExecUnitsMap {
    /// Look up ExUnits for a given (tag, index).
    fn get(&self, tag: &RedeemerTag, index: u32) -> Option<ExUnits> {
        self.entries
            .iter()
            .find(|(t, i, _)| t == tag && *i == index)
            .map(|(_, _, eu)| *eu)
    }
}

/// Parse an exec-units JSON file (output from `tx evaluate`) into an ExecUnitsMap.
///
/// Expected format:
/// ```json
/// {
///   "v": "0.1.0", "status": "ok",
///   "redeemers": [
///     {"tag": "spend", "index": 0, "exec_units": {"cpu": 1050000, "mem": 30200}}
///   ]
/// }
/// ```
pub fn parse_exec_units_file(path: &str) -> Result<ExecUnitsMap, TxBuildError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| TxBuildError::ReadError(format!("exec-units file '{path}': {e}")))?;

    let json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| TxBuildError::InvalidSpec(format!("invalid exec-units JSON: {e}")))?;

    let redeemers = json
        .get("redeemers")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            TxBuildError::InvalidSpec("exec-units file missing 'redeemers' array".to_string())
        })?;

    let mut map = ExecUnitsMap::default();

    for entry in redeemers {
        let tag_str = entry.get("tag").and_then(|v| v.as_str()).ok_or_else(|| {
            TxBuildError::InvalidSpec("exec-units redeemer missing 'tag'".to_string())
        })?;

        let tag = match tag_str {
            "spend" => RedeemerTag::Spend,
            "mint" => RedeemerTag::Mint,
            "cert" | "certificate" => RedeemerTag::Cert,
            "reward" | "withdrawal" | "withdraw" => RedeemerTag::Reward,
            "vote" => RedeemerTag::Vote,
            "propose" => RedeemerTag::Propose,
            _ => {
                return Err(TxBuildError::InvalidSpec(format!(
                    "unknown redeemer tag in exec-units: {tag_str}"
                )));
            }
        };

        let index = entry.get("index").and_then(|v| v.as_u64()).ok_or_else(|| {
            TxBuildError::InvalidSpec("exec-units redeemer missing 'index'".to_string())
        })? as u32;

        let eu = entry.get("exec_units").ok_or_else(|| {
            TxBuildError::InvalidSpec("exec-units redeemer missing 'exec_units'".to_string())
        })?;

        let cpu = eu
            .get("cpu")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| TxBuildError::InvalidSpec("exec-units missing 'cpu'".to_string()))?;

        let mem = eu
            .get("mem")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| TxBuildError::InvalidSpec("exec-units missing 'mem'".to_string()))?;

        map.entries.push((tag, index, ExUnits { mem, steps: cpu }));
    }

    Ok(map)
}

/// Build a full Conway-era Tx and encode it to CBOR bytes.
///
/// Returns `(cbor_bytes, fee, warnings)` where:
/// - `fee` is calculated from the tx size using mainnet protocol parameters
/// - `warnings` contains any advisory messages (e.g., missing input values)
///
/// Parameters:
/// - `spec`: the transaction specification
/// - `network`: target network name ("mainnet", "preview", "preprod", etc.)
/// - `exec_units`: optional exec units from `tx evaluate`
/// - `cost_models`: optional cost models JSON from protocol parameters
///
/// When all inputs have `value` fields, a change output is automatically
/// appended to `change_address`. Otherwise, the caller must ensure balance.
pub fn build_cbor_tx(
    spec: &TxSpec,
    network: &str,
    exec_units: Option<&ExecUnitsMap>,
    cost_models: Option<&serde_json::Value>,
) -> Result<(Vec<u8>, u64, Vec<String>), TxBuildError> {
    let mut warnings = Vec::new();

    // Compute script_data_hash from the witness set
    let witness_set_for_hash = build_witness_set(spec, exec_units)?;
    let (sdh, sdh_warnings) = compute_script_data_hash_for_tx(&witness_set_for_hash, cost_models)?;
    warnings.extend(sdh_warnings);

    // Check if we can compute change
    let can_balance = total_input_lovelace(spec).is_some();
    if !can_balance {
        warnings.push(
            "input values not provided for all inputs; no change output created. \
             Caller must ensure inputs = outputs + fee."
                .to_string(),
        );
    }

    // Compute remaining tokens for the change output
    let input_tokens = sum_input_tokens(spec);
    let output_tokens = sum_output_tokens(spec);
    let change_tokens = remaining_tokens(&input_tokens, &output_tokens);
    if !change_tokens.is_empty() && !can_balance {
        warnings.push(
            "input tokens are not fully consumed by outputs, but input values are incomplete; \
             remaining tokens will be included in the change output with min-UTxO lovelace."
                .to_string(),
        );
    }
    let empty_tokens: TokenMap = BTreeMap::new();

    // First pass: build with a placeholder fee (and no change) to estimate size
    let placeholder_fee: u64 = 200_000;

    let body = build_transaction_body(spec, placeholder_fee, None, &empty_tokens, sdh, network)?;
    let witness_set_pass1 = build_witness_set(spec, exec_units)?;
    let tx: Tx = PseudoTx {
        transaction_body: body,
        transaction_witness_set: witness_set_pass1,
        success: true,
        auxiliary_data: Nullable::Null,
    };

    let first_pass = pallas_codec::minicbor::to_vec(&tx)
        .map_err(|e| TxBuildError::WriteError(format!("CBOR encoding failed: {e}")))?;

    // Calculate fee based on size.
    // NOTE: Fee parameters (44 lovelace/byte + 155381 constant) are current mainnet values.
    // In production these should be fetched from protocol parameters.
    let mut fee = MIN_FEE_COEFFICIENT * (first_pass.len() as u64) + MIN_FEE_CONSTANT;

    // Account for witness overhead: each VKeyWitness adds ~102 bytes
    // (32 vkey + 64 signature + CBOR tags/lengths).
    // At minimum 1 signature is needed; required_signers may add more.
    let num_signers = std::cmp::max(1, spec.required_signers.len() as u64);
    let witness_overhead = num_signers * 102;
    fee += MIN_FEE_COEFFICIENT * witness_overhead;

    // If we can balance, the change output adds ~70 bytes to the tx, adjust fee estimate
    if can_balance {
        fee += MIN_FEE_COEFFICIENT * 70;
    }

    // Compute change output
    let change_lovelace = if can_balance {
        let total_in = total_input_lovelace(spec).ok_or_else(|| {
            TxBuildError::InvalidSpec("unexpected: input values disappeared".to_string())
        })?;
        let total_out = total_output_lovelace(spec);
        let required = total_out
            .checked_add(fee)
            .ok_or_else(|| TxBuildError::InvalidSpec("output + fee overflow".to_string()))?;
        if total_in < required {
            return Err(TxBuildError::InvalidSpec(format!(
                "insufficient input value: inputs have {total_in} lovelace but outputs + fee require {required}"
            )));
        }
        let change = total_in - required;
        if change > 0 {
            if change < MIN_UTXO_LOVELACE {
                // Change is below min-UTxO threshold; absorb into fee instead
                fee += change;
                warnings.push(format!(
                    "change output ({change} lovelace) is below min-UTxO ({MIN_UTXO_LOVELACE} lovelace); absorbed into fee"
                ));
                None
            } else {
                Some(change)
            }
        } else {
            None
        }
    } else {
        None
    };

    // Check that all explicit outputs meet min-UTxO
    for (i, output) in spec.outputs.iter().enumerate() {
        if output.value.lovelace < MIN_UTXO_LOVELACE {
            return Err(TxBuildError::InvalidSpec(format!(
                "outputs[{i}] has {lovelace} lovelace, below the minimum UTxO requirement of {MIN_UTXO_LOVELACE} lovelace (1 ADA)",
                lovelace = output.value.lovelace
            )));
        }
    }

    // Second pass with calculated fee and change output (including remaining tokens)
    let body = build_transaction_body(spec, fee, change_lovelace, &change_tokens, sdh, network)?;
    let witness_set_pass2 = build_witness_set(spec, exec_units)?;
    let tx: Tx = PseudoTx {
        transaction_body: body,
        transaction_witness_set: witness_set_pass2,
        success: true,
        auxiliary_data: Nullable::Null,
    };

    let cbor_bytes = pallas_codec::minicbor::to_vec(&tx)
        .map_err(|e| TxBuildError::WriteError(format!("CBOR encoding failed: {e}")))?;

    Ok((cbor_bytes, fee, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tx::builder::parse_tx_spec;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// Helper: build cbor tx with default test params (preview network, no exec-units, no cost model).
    fn build_cbor_tx_test(spec: &TxSpec) -> Result<(Vec<u8>, u64, Vec<String>), TxBuildError> {
        build_cbor_tx(spec, "preview", None, None)
    }

    /// Helper: build cbor tx with a dummy cost model so script_data_hash gets computed.
    fn build_cbor_tx_with_cost_model(
        spec: &TxSpec,
    ) -> Result<(Vec<u8>, u64, Vec<String>), TxBuildError> {
        let cm = serde_json::json!({"PlutusV3": []});
        build_cbor_tx(spec, "preview", None, Some(&cm))
    }

    /// A spec using valid 64-char hex tx hashes for proper CBOR encoding.
    fn cbor_spec_json() -> &'static str {
        r#"{
            "inputs": [{"utxo": "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd#0", "type": "pubkey"}],
            "script_inputs": [
                {
                    "utxo": "1122334411223344112233441122334411223344112233441122334411223344#1",
                    "validator": "escrow.spend",
                    "purpose": "spend",
                    "datum": {"constructor": 0, "fields": [{"int": 42}]},
                    "redeemer": {"constructor": 0, "fields": []},
                    "datum_source": "inline"
                }
            ],
            "reference_inputs": [],
            "outputs": [
                {"address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp", "value": {"lovelace": 5000000}, "datum": null},
                {"address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp", "value": {"lovelace": 2000000}}
            ],
            "mint": null,
            "collateral": "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd#2",
            "change_address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp",
            "required_signers": ["aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd"],
            "validity": {"from_slot": null, "to_slot": 2000},
            "metadata": null
        }"#
    }

    #[test]
    fn test_parse_utxo_ref_valid() -> TestResult {
        let input =
            parse_utxo_ref("aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd#0")?;
        assert_eq!(input.index, 0);
        Ok(())
    }

    #[test]
    fn test_parse_utxo_ref_short_hash_errors() {
        // Short hash must now produce an error (no more zero-padding)
        let result = parse_utxo_ref("abc123#5");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("exactly 64 hex chars"), "got: {err}");
    }

    #[test]
    fn test_parse_utxo_ref_invalid_format() {
        let result = parse_utxo_ref("nohash");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_address_bech32() -> TestResult {
        let addr = "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp";
        let bytes = decode_address(addr)?;
        assert!(!bytes.is_empty());
        Ok(())
    }

    #[test]
    fn test_decode_address_invalid_errors() {
        // Invalid bech32 and non-hex string must now produce an error (no raw fallback)
        let result = decode_address("addr_test1qz123");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid address"), "got: {err}");
    }

    #[test]
    fn test_build_cbor_tx_basic() -> TestResult {
        let spec = parse_tx_spec(cbor_spec_json())?;
        let (cbor_bytes, fee, _warnings) = build_cbor_tx_with_cost_model(&spec)?;

        assert!(!cbor_bytes.is_empty());
        assert!(fee > 0);

        // Verify we can decode it back with pallas
        let decoded: Tx = pallas_codec::minicbor::decode(&cbor_bytes)
            .map_err(|e| format!("decode failed: {e}"))?;

        // Verify structure
        assert_eq!(decoded.transaction_body.inputs.len(), 2); // 1 pubkey + 1 script
        assert_eq!(decoded.transaction_body.outputs.len(), 2);
        assert_eq!(decoded.transaction_body.fee, fee);
        assert_eq!(decoded.transaction_body.ttl, Some(2000));
        assert!(decoded.transaction_body.collateral.is_some());
        assert!(decoded.transaction_body.required_signers.is_some());
        assert!(decoded.success);

        Ok(())
    }

    #[test]
    fn test_build_cbor_tx_with_mint() -> TestResult {
        let json = r#"{
            "inputs": [{"utxo": "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd#0", "type": "pubkey"}],
            "script_inputs": [
                {
                    "utxo": "1122334411223344112233441122334411223344112233441122334411223344#1",
                    "validator": "escrow.spend",
                    "purpose": "spend",
                    "datum": {"constructor": 0, "fields": []},
                    "redeemer": {"constructor": 0, "fields": []},
                    "datum_source": "inline"
                }
            ],
            "outputs": [
                {"address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp", "value": {"lovelace": 5000000}}
            ],
            "mint": {
                "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd": {
                    "assets": {"4d79546f6b656e": 1},
                    "redeemer": {"constructor": 0, "fields": []},
                    "validator": "token.mint"
                }
            },
            "collateral": "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd#2",
            "change_address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp"
        }"#;
        let spec = parse_tx_spec(json)?;
        let (cbor_bytes, _fee, _warnings) = build_cbor_tx_with_cost_model(&spec)?;

        let decoded: Tx = pallas_codec::minicbor::decode(&cbor_bytes)
            .map_err(|e| format!("decode failed: {e}"))?;

        assert!(decoded.transaction_body.mint.is_some());

        // Check witness set has redeemers
        assert!(decoded.transaction_witness_set.redeemer.is_some());

        Ok(())
    }

    #[test]
    fn test_build_cbor_tx_roundtrip_hex() -> TestResult {
        let spec = parse_tx_spec(cbor_spec_json())?;
        let (cbor_bytes, _fee, _warnings) = build_cbor_tx_test(&spec)?;

        // Hex encode then decode
        let hex_str = hex::encode(&cbor_bytes);
        let decoded_bytes = hex::decode(&hex_str)?;
        assert_eq!(cbor_bytes, decoded_bytes);

        // Decode from hex-decoded bytes
        let _decoded: Tx = pallas_codec::minicbor::decode(&decoded_bytes)
            .map_err(|e| format!("decode failed: {e}"))?;

        Ok(())
    }

    #[test]
    fn test_build_cbor_tx_simple_pubkey_only() -> TestResult {
        let json = r#"{
            "inputs": [{"utxo": "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd#0", "type": "pubkey"}],
            "outputs": [
                {"address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp", "value": {"lovelace": 5000000}}
            ],
            "change_address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp"
        }"#;
        let spec = parse_tx_spec(json)?;
        let (cbor_bytes, fee, _warnings) = build_cbor_tx_test(&spec)?;

        let decoded: Tx = pallas_codec::minicbor::decode(&cbor_bytes)
            .map_err(|e| format!("decode failed: {e}"))?;

        assert_eq!(decoded.transaction_body.inputs.len(), 1);
        assert_eq!(decoded.transaction_body.outputs.len(), 1);
        assert_eq!(decoded.transaction_body.fee, fee);
        assert!(decoded.transaction_body.collateral.is_none());
        assert!(decoded.transaction_body.required_signers.is_none());
        assert!(decoded.transaction_body.mint.is_none());
        assert!(decoded.transaction_witness_set.redeemer.is_none());

        Ok(())
    }

    #[test]
    fn test_fee_estimation_based_on_size() -> TestResult {
        let json = r#"{
            "inputs": [{"utxo": "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd#0", "type": "pubkey"}],
            "outputs": [
                {"address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp", "value": {"lovelace": 5000000}}
            ],
            "change_address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp"
        }"#;
        let spec = parse_tx_spec(json)?;
        let (cbor_bytes, fee, _warnings) = build_cbor_tx_test(&spec)?;

        // Fee should be approximately: 44 * (size + witness_overhead) + 155381
        // Witness overhead: at least 1 signer × 102 bytes
        let witness_bytes = 102_u64;
        let expected_approx =
            MIN_FEE_COEFFICIENT * (cbor_bytes.len() as u64 + witness_bytes) + MIN_FEE_CONSTANT;
        let diff = if fee > expected_approx {
            fee - expected_approx
        } else {
            expected_approx - fee
        };
        assert!(diff < 500, "fee estimation off by {diff} lovelace");

        Ok(())
    }

    #[test]
    fn test_script_data_hash_set_when_scripts_present() -> TestResult {
        let spec = parse_tx_spec(cbor_spec_json())?;
        let (cbor_bytes, _fee, _warnings) = build_cbor_tx_with_cost_model(&spec)?;

        let decoded: Tx = pallas_codec::minicbor::decode(&cbor_bytes)
            .map_err(|e| format!("decode failed: {e}"))?;

        // script_data_hash should be set because there are script inputs with redeemers
        assert!(
            decoded.transaction_body.script_data_hash.is_some(),
            "script_data_hash should be set when scripts are present"
        );

        Ok(())
    }

    #[test]
    fn test_script_data_hash_none_when_no_scripts() -> TestResult {
        let json = r#"{
            "inputs": [{"utxo": "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd#0", "type": "pubkey"}],
            "outputs": [
                {"address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp", "value": {"lovelace": 5000000}}
            ],
            "change_address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp"
        }"#;
        let spec = parse_tx_spec(json)?;
        let (cbor_bytes, _fee, _warnings) = build_cbor_tx_test(&spec)?;

        let decoded: Tx = pallas_codec::minicbor::decode(&cbor_bytes)
            .map_err(|e| format!("decode failed: {e}"))?;

        assert!(
            decoded.transaction_body.script_data_hash.is_none(),
            "script_data_hash should be None when no scripts are present"
        );

        Ok(())
    }

    #[test]
    fn test_change_output_created_with_input_values() -> TestResult {
        let json = r#"{
            "inputs": [
                {"utxo": "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd#0", "type": "pubkey", "value": {"lovelace": 20000000}}
            ],
            "outputs": [
                {"address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp", "value": {"lovelace": 5000000}}
            ],
            "change_address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp"
        }"#;
        let spec = parse_tx_spec(json)?;
        let (cbor_bytes, fee, warnings) = build_cbor_tx_test(&spec)?;

        // No warnings when input values are provided
        assert!(
            warnings.is_empty(),
            "expected no warnings but got: {warnings:?}"
        );

        let decoded: Tx = pallas_codec::minicbor::decode(&cbor_bytes)
            .map_err(|e| format!("decode failed: {e}"))?;

        // Should have 2 outputs: the explicit one + change
        assert_eq!(
            decoded.transaction_body.outputs.len(),
            2,
            "expected 2 outputs (1 explicit + 1 change)"
        );

        // Verify balance: input = outputs + fee
        let total_in: u64 = 20_000_000;
        let explicit_out: u64 = 5_000_000;
        let change = total_in - explicit_out - fee;
        // The second output should be the change output
        match &decoded.transaction_body.outputs[1] {
            PseudoTransactionOutput::PostAlonzo(o) => {
                if let Value::Coin(coin) = o.value {
                    assert_eq!(
                        coin, change,
                        "change output should be {change} but was {coin}"
                    );
                } else {
                    return Err("expected Coin value for change output".into());
                }
            }
            _ => return Err("expected PostAlonzo output".into()),
        }

        Ok(())
    }

    #[test]
    fn test_no_change_output_without_input_values() -> TestResult {
        // No input values => no change output, but a warning
        let json = r#"{
            "inputs": [{"utxo": "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd#0", "type": "pubkey"}],
            "outputs": [
                {"address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp", "value": {"lovelace": 5000000}}
            ],
            "change_address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp"
        }"#;
        let spec = parse_tx_spec(json)?;
        let (_cbor_bytes, _fee, warnings) = build_cbor_tx_test(&spec)?;

        assert!(
            !warnings.is_empty(),
            "expected a warning about missing input values"
        );
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("input values not provided")),
            "expected warning about input values, got: {warnings:?}"
        );

        Ok(())
    }

    #[test]
    fn test_insufficient_input_value_returns_error() -> TestResult {
        let json = r#"{
            "inputs": [
                {"utxo": "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd#0", "type": "pubkey", "value": {"lovelace": 100}}
            ],
            "outputs": [
                {"address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp", "value": {"lovelace": 5000000}}
            ],
            "change_address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp"
        }"#;
        let spec = parse_tx_spec(json)?;
        let result = build_cbor_tx_test(&spec);
        assert!(
            result.is_err(),
            "expected error for insufficient input value"
        );
        let err = result.err().ok_or("expected error")?.to_string();
        assert!(
            err.contains("insufficient input value"),
            "expected 'insufficient input value' error, got: {err}"
        );
        Ok(())
    }

    #[test]
    fn test_change_output_with_script_inputs_and_values() -> TestResult {
        let json = r#"{
            "inputs": [
                {"utxo": "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd#0", "type": "pubkey", "value": {"lovelace": 10000000}}
            ],
            "script_inputs": [
                {
                    "utxo": "1122334411223344112233441122334411223344112233441122334411223344#1",
                    "validator": "escrow.spend",
                    "purpose": "spend",
                    "datum": {"constructor": 0, "fields": [{"int": 42}]},
                    "redeemer": {"constructor": 0, "fields": []},
                    "datum_source": "inline",
                    "value": {"lovelace": 15000000}
                }
            ],
            "outputs": [
                {"address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp", "value": {"lovelace": 5000000}}
            ],
            "collateral": "aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd#2",
            "change_address": "addr_test1qz2fxv2umyhttkxyxp8x0dlpdt3k6cwng5pxj3jhsydzer3jcu5d8ps7zex2k2xt3uqxgjqnnj83ws8lhrn648jjxtwq2ytjqp",
            "required_signers": ["aabbccddaabbccddaabbccddaabbccddaabbccddaabbccddaabbccdd"]
        }"#;
        let spec = parse_tx_spec(json)?;
        let (cbor_bytes, fee, warnings) = build_cbor_tx_with_cost_model(&spec)?;

        assert!(warnings.is_empty(), "expected no warnings: {warnings:?}");

        let decoded: Tx = pallas_codec::minicbor::decode(&cbor_bytes)
            .map_err(|e| format!("decode failed: {e}"))?;

        // 2 outputs: explicit + change
        assert_eq!(decoded.transaction_body.outputs.len(), 2);

        // script_data_hash should also be set (cost model provided)
        assert!(decoded.transaction_body.script_data_hash.is_some());

        // Verify balance
        let total_in: u64 = 25_000_000;
        let explicit_out: u64 = 5_000_000;
        let expected_change = total_in - explicit_out - fee;
        match &decoded.transaction_body.outputs[1] {
            PseudoTransactionOutput::PostAlonzo(o) => {
                if let Value::Coin(coin) = o.value {
                    assert_eq!(coin, expected_change);
                } else {
                    return Err("expected Coin value".into());
                }
            }
            _ => return Err("expected PostAlonzo output".into()),
        }

        Ok(())
    }
}
