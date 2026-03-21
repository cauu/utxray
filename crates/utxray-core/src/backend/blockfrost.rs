use std::collections::HashMap;

use anyhow::{anyhow, bail, Context};
use serde::Deserialize;

use super::{
    DatumInfo, EvaluatedRedeemer, EvaluationResult, ExUnits, TipInfo, UtxoInfo, UtxoValue,
};

/// Concrete Blockfrost HTTP backend (no trait abstraction per architecture rules).
pub struct BlockfrostBackend {
    client: reqwest::Client,
    base_url: String,
    project_id: String,
}

// ---------------------------------------------------------------------------
// Blockfrost JSON response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct BlockfrostUtxo {
    pub tx_hash: String,
    pub tx_index: u32,
    pub output_index: u32,
    pub amount: Vec<BlockfrostAmount>,
    pub block: String,
    pub data_hash: Option<String>,
    pub inline_datum: Option<serde_json::Value>,
    pub reference_script_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BlockfrostAmount {
    pub unit: String,
    pub quantity: String,
}

#[derive(Debug, Deserialize)]
pub struct BlockfrostDatumResponse {
    pub json_value: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct BlockfrostTipResponse {
    pub slot: Option<u64>,
    pub hash: Option<String>,
    pub height: Option<u64>,
    pub epoch: Option<u64>,
    pub time: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct BlockfrostHealthResponse {
    pub is_healthy: bool,
}

/// Blockfrost error payload
#[derive(Debug, Deserialize)]
pub struct BlockfrostErrorResponse {
    pub status_code: u16,
    pub error: String,
    pub message: String,
}

/// Blockfrost Ogmios-style evaluation result
#[derive(Debug, Deserialize)]
pub struct BlockfrostEvalResult {
    pub result: Option<BlockfrostEvalOk>,
    pub error: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct BlockfrostEvalOk {
    #[serde(rename = "EvaluationResult")]
    pub evaluation_result: Option<HashMap<String, BlockfrostEvalRedeemer>>,
}

#[derive(Debug, Deserialize)]
pub struct BlockfrostEvalRedeemer {
    pub memory: u64,
    pub steps: u64,
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl BlockfrostBackend {
    /// Create a new Blockfrost backend for the given network.
    ///
    /// `network` must be one of: `preview`, `preprod`, `mainnet`.
    pub fn new(project_id: &str, network: &str) -> anyhow::Result<Self> {
        let base_url = network_to_base_url(network)?;
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self {
            client,
            base_url,
            project_id: project_id.to_string(),
        })
    }

    /// Check Blockfrost health endpoint.
    pub async fn health(&self) -> anyhow::Result<bool> {
        let url = format!("{}/health", self.base_url);
        let resp = self
            .client
            .get(&url)
            .header("project_id", &self.project_id)
            .send()
            .await
            .context("blockfrost health request failed")?;

        if !resp.status().is_success() {
            return Ok(false);
        }

        let body: BlockfrostHealthResponse = resp
            .json()
            .await
            .context("failed to parse health response")?;
        Ok(body.is_healthy)
    }

    /// Query all UTxOs at the given address (handles pagination).
    pub async fn query_utxos(&self, address: &str) -> anyhow::Result<Vec<UtxoInfo>> {
        let mut all_utxos: Vec<UtxoInfo> = Vec::new();
        let mut page: u32 = 1;

        loop {
            let url = format!(
                "{}/addresses/{}/utxos?page={}&count=100",
                self.base_url, address, page
            );

            let resp = self
                .client
                .get(&url)
                .header("project_id", &self.project_id)
                .send()
                .await
                .context("blockfrost utxo query request failed")?;

            let status = resp.status();
            if status == reqwest::StatusCode::NOT_FOUND {
                // Address has no UTxOs — return empty
                return Ok(all_utxos);
            }
            if !status.is_success() {
                return Err(map_http_error(
                    status.as_u16(),
                    &resp.text().await.unwrap_or_default(),
                ));
            }

            let batch: Vec<BlockfrostUtxo> =
                resp.json().await.context("failed to parse utxo response")?;

            let batch_len = batch.len();

            for bf in batch {
                all_utxos.push(convert_utxo(address, bf));
            }

            if batch_len < 100 {
                break;
            }
            page += 1;
        }

        Ok(all_utxos)
    }

    /// Resolve an on-chain datum by its hash.
    pub async fn resolve_datum(&self, datum_hash: &str) -> anyhow::Result<Option<DatumInfo>> {
        let url = format!("{}/scripts/datum/{}", self.base_url, datum_hash);

        let resp = self
            .client
            .get(&url)
            .header("project_id", &self.project_id)
            .send()
            .await
            .context("blockfrost datum resolve request failed")?;

        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(map_http_error(
                status.as_u16(),
                &resp.text().await.unwrap_or_default(),
            ));
        }

        let body: BlockfrostDatumResponse = resp
            .json()
            .await
            .context("failed to parse datum response")?;

        Ok(Some(DatumInfo {
            hash: datum_hash.to_string(),
            source: "indexer".to_string(),
            decoded: body.json_value,
        }))
    }

    /// Evaluate a transaction using Blockfrost's Ogmios-compatible endpoint.
    ///
    /// `tx_cbor_hex` is the hex-encoded CBOR of the transaction.
    pub async fn evaluate_tx(&self, tx_cbor_hex: &str) -> anyhow::Result<EvaluationResult> {
        let url = format!("{}/utils/txs/evaluate", self.base_url);

        let cbor_bytes = hex::decode(tx_cbor_hex).context("invalid hex in tx CBOR for evaluate")?;

        let resp = self
            .client
            .post(&url)
            .header("project_id", &self.project_id)
            .header("Content-Type", "application/cbor")
            .body(cbor_bytes)
            .send()
            .await
            .context("blockfrost tx evaluate request failed")?;

        let status = resp.status();
        if !status.is_success() {
            return Err(map_http_error(
                status.as_u16(),
                &resp.text().await.unwrap_or_default(),
            ));
        }

        let body: BlockfrostEvalResult = resp
            .json()
            .await
            .context("failed to parse evaluate response")?;

        if let Some(err) = body.error {
            bail!("evaluation error: {}", err);
        }

        let eval_ok = body
            .result
            .ok_or_else(|| anyhow!("evaluation response missing result field"))?;

        let eval_map = eval_ok
            .evaluation_result
            .ok_or_else(|| anyhow!("evaluation response missing EvaluationResult field"))?;

        let mut redeemers: Vec<EvaluatedRedeemer> = Vec::new();
        for (key, val) in eval_map {
            // Keys are like "spend:0", "mint:1", etc.
            let parts: Vec<&str> = key.splitn(2, ':').collect();
            let (tag, index) = if parts.len() == 2 {
                let idx = parts[1].parse::<u32>().unwrap_or(0);
                (parts[0].to_string(), idx)
            } else {
                (key.clone(), 0)
            };

            redeemers.push(EvaluatedRedeemer {
                tag,
                index,
                exec_units: ExUnits {
                    cpu: val.steps,
                    mem: val.memory,
                },
            });
        }

        // Sort for deterministic output
        redeemers.sort_by(|a, b| a.tag.cmp(&b.tag).then(a.index.cmp(&b.index)));

        Ok(EvaluationResult { redeemers })
    }

    /// Submit a signed transaction to the network.
    ///
    /// Returns the transaction hash on success.
    pub async fn submit_tx(&self, tx_cbor_hex: &str) -> anyhow::Result<String> {
        let url = format!("{}/tx/submit", self.base_url);

        let cbor_bytes = hex::decode(tx_cbor_hex).context("invalid hex in tx CBOR for submit")?;

        let resp = self
            .client
            .post(&url)
            .header("project_id", &self.project_id)
            .header("Content-Type", "application/cbor")
            .body(cbor_bytes)
            .send()
            .await
            .context("blockfrost tx submit request failed")?;

        let status = resp.status();
        if !status.is_success() {
            return Err(map_http_error(
                status.as_u16(),
                &resp.text().await.unwrap_or_default(),
            ));
        }

        // Blockfrost returns the tx hash as a JSON string
        let tx_hash: String = resp
            .json()
            .await
            .context("failed to parse submit response")?;

        Ok(tx_hash)
    }

    /// Query the latest protocol parameters.
    pub async fn query_params(&self) -> anyhow::Result<serde_json::Value> {
        let url = format!("{}/epochs/latest/parameters", self.base_url);

        let resp = self
            .client
            .get(&url)
            .header("project_id", &self.project_id)
            .send()
            .await
            .context("blockfrost params query request failed")?;

        let status = resp.status();
        if !status.is_success() {
            return Err(map_http_error(
                status.as_u16(),
                &resp.text().await.unwrap_or_default(),
            ));
        }

        let params: serde_json::Value = resp
            .json()
            .await
            .context("failed to parse params response")?;

        Ok(params)
    }

    /// Query the latest chain tip.
    pub async fn query_tip(&self) -> anyhow::Result<TipInfo> {
        let url = format!("{}/blocks/latest", self.base_url);

        let resp = self
            .client
            .get(&url)
            .header("project_id", &self.project_id)
            .send()
            .await
            .context("blockfrost tip query request failed")?;

        let status = resp.status();
        if !status.is_success() {
            return Err(map_http_error(
                status.as_u16(),
                &resp.text().await.unwrap_or_default(),
            ));
        }

        let body: BlockfrostTipResponse =
            resp.json().await.context("failed to parse tip response")?;

        Ok(TipInfo {
            slot: body.slot.unwrap_or(0),
            block_hash: body.hash.unwrap_or_default(),
            block_height: body.height.unwrap_or(0),
            epoch: body.epoch.unwrap_or(0),
            // Blockfrost timestamps are in seconds; convert to milliseconds
            time_s: body.time.unwrap_or(0),
        })
    }

    /// Returns the base URL used by this backend.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn network_to_base_url(network: &str) -> anyhow::Result<String> {
    match network {
        "preview" => Ok("https://cardano-preview.blockfrost.io/api/v0".to_string()),
        "preprod" => Ok("https://cardano-preprod.blockfrost.io/api/v0".to_string()),
        "mainnet" => Ok("https://cardano-mainnet.blockfrost.io/api/v0".to_string()),
        other => Err(anyhow!(
            "unsupported network '{}': expected preview, preprod, or mainnet",
            other
        )),
    }
}

fn map_http_error(status: u16, body: &str) -> anyhow::Error {
    // Try to parse the Blockfrost error JSON for a friendlier message
    let detail = serde_json::from_str::<BlockfrostErrorResponse>(body)
        .map(|e| format!("{}: {}", e.error, e.message))
        .unwrap_or_else(|_| body.to_string());

    match status {
        400 => anyhow!("blockfrost bad request (400): {}", detail),
        402 => anyhow!("blockfrost usage limit exceeded (402): {}", detail),
        403 => anyhow!(
            "blockfrost auth error (403): invalid project_id or access denied: {}",
            detail
        ),
        404 => anyhow!("blockfrost not found (404): {}", detail),
        429 => anyhow!("blockfrost rate limit exceeded (429): {}", detail),
        500 => anyhow!("blockfrost server error (500): {}", detail),
        _ => anyhow!("blockfrost HTTP error ({}): {}", status, detail),
    }
}

fn convert_utxo(address: &str, bf: BlockfrostUtxo) -> UtxoInfo {
    let mut lovelace: u64 = 0;
    let mut tokens: HashMap<String, HashMap<String, u64>> = HashMap::new();

    for amt in &bf.amount {
        if amt.unit == "lovelace" {
            lovelace = amt.quantity.parse::<u64>().unwrap_or(0);
        } else {
            // unit format: <policy_id><asset_name_hex> (56 chars policy + rest is asset name)
            let (policy_id, asset_name) = if amt.unit.len() > 56 {
                (amt.unit[..56].to_string(), amt.unit[56..].to_string())
            } else {
                (amt.unit.clone(), String::new())
            };
            let qty = amt.quantity.parse::<u64>().unwrap_or(0);
            tokens.entry(policy_id).or_default().insert(asset_name, qty);
        }
    }

    UtxoInfo {
        tx_hash: bf.tx_hash,
        index: bf.output_index,
        value: UtxoValue { lovelace, tokens },
        address: address.to_string(),
        datum_hash: bf.data_hash,
        inline_datum: bf.inline_datum,
        reference_script_hash: bf.reference_script_hash,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn test_network_to_base_url_preview() -> TestResult {
        let url = network_to_base_url("preview")?;
        assert_eq!(url, "https://cardano-preview.blockfrost.io/api/v0");
        Ok(())
    }

    #[test]
    fn test_network_to_base_url_preprod() -> TestResult {
        let url = network_to_base_url("preprod")?;
        assert_eq!(url, "https://cardano-preprod.blockfrost.io/api/v0");
        Ok(())
    }

    #[test]
    fn test_network_to_base_url_mainnet() -> TestResult {
        let url = network_to_base_url("mainnet")?;
        assert_eq!(url, "https://cardano-mainnet.blockfrost.io/api/v0");
        Ok(())
    }

    #[test]
    fn test_network_to_base_url_invalid() {
        let result = network_to_base_url("testnet");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("unsupported network"));
    }

    #[test]
    fn test_new_creates_backend() -> TestResult {
        let backend = BlockfrostBackend::new("test_project_id", "preview")?;
        assert_eq!(
            backend.base_url(),
            "https://cardano-preview.blockfrost.io/api/v0"
        );
        Ok(())
    }

    #[test]
    fn test_new_invalid_network() {
        let result = BlockfrostBackend::new("test_id", "invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_map_http_error_codes() {
        let err400 = map_http_error(
            400,
            r#"{"status_code":400,"error":"Bad Request","message":"invalid address"}"#,
        );
        assert!(err400.to_string().contains("bad request"));
        assert!(err400.to_string().contains("invalid address"));

        let err402 = map_http_error(
            402,
            r#"{"status_code":402,"error":"Usage Limit","message":"over limit"}"#,
        );
        assert!(err402.to_string().contains("usage limit"));

        let err403 = map_http_error(
            403,
            r#"{"status_code":403,"error":"Forbidden","message":"bad key"}"#,
        );
        assert!(err403.to_string().contains("auth error"));

        let err404 = map_http_error(
            404,
            r#"{"status_code":404,"error":"Not Found","message":"no datum"}"#,
        );
        assert!(err404.to_string().contains("not found"));

        let err429 = map_http_error(
            429,
            r#"{"status_code":429,"error":"Rate Limit","message":"slow down"}"#,
        );
        assert!(err429.to_string().contains("rate limit"));

        let err500 = map_http_error(
            500,
            r#"{"status_code":500,"error":"Server Error","message":"oops"}"#,
        );
        assert!(err500.to_string().contains("server error"));

        let err503 = map_http_error(503, "service unavailable");
        assert!(err503.to_string().contains("503"));
    }

    #[test]
    fn test_map_http_error_invalid_json() {
        let err = map_http_error(400, "not json");
        assert!(err.to_string().contains("not json"));
    }

    #[test]
    fn test_deserialize_utxo_response() -> TestResult {
        let fixture = include_str!("../../../../tests/fixtures/blockfrost/utxos_response.json");
        let utxos: Vec<BlockfrostUtxo> = serde_json::from_str(fixture)?;
        assert_eq!(utxos.len(), 2);
        assert_eq!(
            utxos[0].tx_hash,
            "aaaa000000000000000000000000000000000000000000000000000000000000"
        );
        assert_eq!(utxos[0].output_index, 0);
        assert_eq!(utxos[0].amount.len(), 2);
        assert_eq!(utxos[0].amount[0].unit, "lovelace");
        assert_eq!(utxos[0].amount[0].quantity, "5000000");
        assert!(utxos[0].data_hash.is_some());
        assert_eq!(utxos[1].output_index, 1);
        assert!(utxos[1].inline_datum.is_some());
        Ok(())
    }

    #[test]
    fn test_deserialize_datum_response() -> TestResult {
        let fixture = include_str!("../../../../tests/fixtures/blockfrost/datum_response.json");
        let datum: BlockfrostDatumResponse = serde_json::from_str(fixture)?;
        assert!(datum.json_value.is_object());
        Ok(())
    }

    #[test]
    fn test_deserialize_tip_response() -> TestResult {
        let fixture = include_str!("../../../../tests/fixtures/blockfrost/tip_response.json");
        let tip: BlockfrostTipResponse = serde_json::from_str(fixture)?;
        assert_eq!(tip.slot, Some(12345678));
        assert_eq!(tip.height, Some(987654));
        assert_eq!(tip.epoch, Some(100));
        Ok(())
    }

    #[test]
    fn test_deserialize_params_response() -> TestResult {
        let fixture = include_str!("../../../../tests/fixtures/blockfrost/params_response.json");
        let params: serde_json::Value = serde_json::from_str(fixture)?;
        assert!(params.get("min_fee_a").is_some());
        assert!(params.get("min_fee_b").is_some());
        Ok(())
    }

    #[test]
    fn test_convert_utxo_lovelace_only() {
        let bf = BlockfrostUtxo {
            tx_hash: "aabb".to_string(),
            tx_index: 0,
            output_index: 0,
            amount: vec![BlockfrostAmount {
                unit: "lovelace".to_string(),
                quantity: "2000000".to_string(),
            }],
            block: "block1".to_string(),
            data_hash: None,
            inline_datum: None,
            reference_script_hash: None,
        };
        let utxo = convert_utxo("addr_test1abc", bf);
        assert_eq!(utxo.value.lovelace, 2_000_000);
        assert!(utxo.value.tokens.is_empty());
        assert_eq!(utxo.address, "addr_test1abc");
    }

    #[test]
    fn test_convert_utxo_with_tokens() -> TestResult {
        let policy = "a".repeat(56);
        let asset_name = "4d79546f6b656e"; // hex for "MyToken"
        let unit = format!("{}{}", policy, asset_name);

        let bf = BlockfrostUtxo {
            tx_hash: "ccdd".to_string(),
            tx_index: 0,
            output_index: 1,
            amount: vec![
                BlockfrostAmount {
                    unit: "lovelace".to_string(),
                    quantity: "1500000".to_string(),
                },
                BlockfrostAmount {
                    unit: unit.clone(),
                    quantity: "42".to_string(),
                },
            ],
            block: "block2".to_string(),
            data_hash: Some("datumhash123".to_string()),
            inline_datum: None,
            reference_script_hash: None,
        };
        let utxo = convert_utxo("addr_test1xyz", bf);
        assert_eq!(utxo.value.lovelace, 1_500_000);
        assert_eq!(utxo.value.tokens.len(), 1);
        let policy_map = utxo
            .value
            .tokens
            .get(&policy)
            .ok_or("policy should exist")?;
        assert_eq!(policy_map.get(asset_name), Some(&42));
        assert_eq!(utxo.datum_hash, Some("datumhash123".to_string()));
        Ok(())
    }

    #[test]
    fn test_deserialize_eval_result() -> TestResult {
        let json_str = r#"{
            "result": {
                "EvaluationResult": {
                    "spend:0": {"memory": 1000, "steps": 2000},
                    "mint:0": {"memory": 500, "steps": 800}
                }
            },
            "error": null
        }"#;
        let parsed: BlockfrostEvalResult = serde_json::from_str(json_str)?;
        assert!(parsed.result.is_some());
        assert!(parsed.error.is_none());
        let eval = parsed.result.ok_or("expected result")?;
        let map = eval.evaluation_result.ok_or("expected eval result")?;
        assert_eq!(map.len(), 2);
        let spend0 = map.get("spend:0").ok_or("expected spend:0")?;
        assert_eq!(spend0.memory, 1000);
        assert_eq!(spend0.steps, 2000);
        Ok(())
    }

    #[test]
    fn test_deserialize_health_response() -> TestResult {
        let json_str = r#"{"is_healthy": true}"#;
        let parsed: BlockfrostHealthResponse = serde_json::from_str(json_str)?;
        assert!(parsed.is_healthy);
        Ok(())
    }

    #[test]
    fn test_deserialize_error_response() -> TestResult {
        let json_str =
            r#"{"status_code": 403, "error": "Forbidden", "message": "Invalid project token."}"#;
        let parsed: BlockfrostErrorResponse = serde_json::from_str(json_str)?;
        assert_eq!(parsed.status_code, 403);
        assert_eq!(parsed.error, "Forbidden");
        Ok(())
    }

    // Integration tests that require a real Blockfrost project ID
    #[tokio::test]
    #[ignore]
    async fn test_health_live() -> TestResult {
        let project_id = std::env::var("BLOCKFROST_PROJECT_ID")?;
        let backend = BlockfrostBackend::new(&project_id, "preview")?;
        let healthy = backend.health().await?;
        assert!(healthy);
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_query_tip_live() -> TestResult {
        let project_id = std::env::var("BLOCKFROST_PROJECT_ID")?;
        let backend = BlockfrostBackend::new(&project_id, "preview")?;
        let tip = backend.query_tip().await?;
        assert!(tip.slot > 0);
        assert!(!tip.block_hash.is_empty());
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_query_params_live() -> TestResult {
        let project_id = std::env::var("BLOCKFROST_PROJECT_ID")?;
        let backend = BlockfrostBackend::new(&project_id, "preview")?;
        let params = backend.query_params().await?;
        assert!(params.get("min_fee_a").is_some());
        Ok(())
    }
}
