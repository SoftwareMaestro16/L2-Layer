use super::{
    FinalizeBatchSignRequest, FinalizeBatchSigner, FinalizerError, OnchainBatchCommitment,
    SignedFinalizeBatch, TonFinalizerProvider,
};
use crate::config::{NodeConfig, SecretString};
use crate::relayer::TonSubmitResult;
use async_trait::async_trait;
use base64::prelude::{Engine as _, BASE64_STANDARD, BASE64_URL_SAFE_NO_PAD};
use l2_core::Hash32;
use serde::Deserialize;
use serde_json::Value;
use tonlib_core::cell::BagOfCells;

#[derive(Clone, Debug)]
pub struct RemoteFinalizeBatchSigner {
    endpoint: String,
    token: SecretString,
    client: reqwest::Client,
}

impl RemoteFinalizeBatchSigner {
    pub fn from_config(config: &NodeConfig) -> Option<Self> {
        Some(Self {
            endpoint: config.l1_commit_signer_endpoint.clone()?,
            token: config.l1_commit_signer_token.clone()?,
            client: reqwest::Client::new(),
        })
    }
}

#[async_trait]
impl FinalizeBatchSigner for RemoteFinalizeBatchSigner {
    async fn sign_finalize_batch(
        &self,
        request: FinalizeBatchSignRequest,
    ) -> Result<SignedFinalizeBatch, FinalizerError> {
        let response = self
            .client
            .post(&self.endpoint)
            .bearer_auth(self.token.expose())
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<RemoteSignerResponse>()
            .await?;
        Ok(SignedFinalizeBatch {
            boc_base64: response.boc_base64,
            signer_address: response.signer_address,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RemoteSignerResponse {
    boc_base64: String,
    signer_address: String,
}

#[derive(Clone, Debug)]
pub struct ToncenterFinalizerProvider {
    base_url: String,
    api_key: SecretString,
    rollup_root_address: String,
    client: reqwest::Client,
}

impl ToncenterFinalizerProvider {
    pub fn from_config(config: &NodeConfig) -> Self {
        Self {
            base_url: config
                .toncenter_v3_base_url
                .trim_end_matches('/')
                .to_owned(),
            api_key: config.toncenter_api_key.clone(),
            rollup_root_address: config.l1_rollup_root_address.clone().unwrap_or_default(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TonFinalizerProvider for ToncenterFinalizerProvider {
    async fn send_signed_boc(
        &self,
        signed: &SignedFinalizeBatch,
    ) -> Result<TonSubmitResult, FinalizerError> {
        let response = self
            .client
            .post(format!("{}/message", self.base_url))
            .header("X-API-Key", self.api_key.expose())
            .json(&serde_json::json!({ "boc": signed.boc_base64 }))
            .send()
            .await?
            .error_for_status()?
            .json::<ToncenterSendMessageResponse>()
            .await?;
        Ok(TonSubmitResult {
            message_hash: parse_hash_or_base64(&response.message_hash)?,
            message_hash_norm: parse_hash_or_base64(&response.message_hash_norm)?,
        })
    }

    async fn message_confirmed(&self, message_hash: Hash32) -> Result<bool, FinalizerError> {
        let response = self
            .client
            .get(format!("{}/transactionsByMessage", self.base_url))
            .header("X-API-Key", self.api_key.expose())
            .query(&[
                ("msg_hash", message_hash.to_hex()),
                ("direction", "in".to_owned()),
                ("limit", "1".to_owned()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<ToncenterTransactionsByMessageResponse>()
            .await?;
        Ok(!response.transactions.is_empty())
    }

    async fn commitment(&self, batch_no: u64) -> Result<OnchainBatchCommitment, FinalizerError> {
        let response = self
            .client
            .post(format!("{}/runGetMethod", self.base_url))
            .header("X-API-Key", self.api_key.expose())
            .json(&serde_json::json!({
                "address": self.rollup_root_address,
                "method": "commitment",
                "stack": [{ "type": "num", "value": batch_no.to_string() }],
            }))
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;
        parse_commitment_response(&response)
    }
}

#[derive(Debug, Deserialize)]
struct ToncenterSendMessageResponse {
    message_hash: String,
    message_hash_norm: String,
}

#[derive(Debug, Deserialize)]
struct ToncenterTransactionsByMessageResponse {
    #[serde(default)]
    transactions: Vec<Value>,
}

pub(crate) fn parse_commitment_response(
    response: &Value,
) -> Result<OnchainBatchCommitment, FinalizerError> {
    let stack = response
        .get("stack")
        .or_else(|| response.pointer("/result/stack"))
        .and_then(Value::as_array)
        .ok_or(FinalizerError::Decode("commitment getter stack missing"))?;
    let exists = stack
        .first()
        .ok_or(FinalizerError::Decode("commitment exists missing"))
        .and_then(parse_stack_bool)?;
    if !exists {
        return Ok(OnchainBatchCommitment {
            exists: false,
            committed_at: None,
            finalized: false,
        });
    }
    let cell_boc = stack
        .get(1)
        .and_then(extract_stack_string)
        .ok_or(FinalizerError::Decode("commitment cell missing"))?;
    let cell = BagOfCells::parse_base64(cell_boc)
        .and_then(BagOfCells::single_root)
        .map_err(|_| FinalizerError::Decode("bad commitment cell"))?;
    let mut parser = cell.parser();
    parser
        .next_reference()
        .map_err(|_| FinalizerError::Decode("commitment rootsA missing"))?;
    parser
        .next_reference()
        .map_err(|_| FinalizerError::Decode("commitment rootsB missing"))?;
    let committed_at = parser
        .load_u32(32)
        .map_err(|_| FinalizerError::Decode("commitment committedAt missing"))?;
    let finalized = parser
        .load_bit()
        .map_err(|_| FinalizerError::Decode("commitment finalized missing"))?;
    Ok(OnchainBatchCommitment {
        exists: true,
        committed_at: Some(u64::from(committed_at)),
        finalized,
    })
}

fn parse_stack_bool(value: &Value) -> Result<bool, FinalizerError> {
    match value {
        Value::Bool(value) => Ok(*value),
        Value::Number(value) => Ok(value.as_i64().unwrap_or_default() != 0),
        Value::String(value) => parse_num_bool(value),
        Value::Array(values) => values
            .get(1)
            .ok_or(FinalizerError::Decode("stack num missing"))
            .and_then(parse_stack_bool),
        Value::Object(map) => map
            .get("value")
            .or_else(|| map.get("num"))
            .or_else(|| map.get("number"))
            .ok_or(FinalizerError::Decode("stack num missing"))
            .and_then(parse_stack_bool),
        Value::Null => Err(FinalizerError::Decode("stack num null")),
    }
}

fn parse_num_bool(value: &str) -> Result<bool, FinalizerError> {
    let value = value.trim();
    if value == "0" || value == "0x0" {
        return Ok(false);
    }
    if value == "-1" || value == "1" || value == "true" {
        return Ok(true);
    }
    if let Some(hex) = value.strip_prefix("0x") {
        return u128::from_str_radix(hex, 16)
            .map(|value| value != 0)
            .map_err(|_| FinalizerError::Decode("bad stack num"));
    }
    value
        .parse::<i128>()
        .map(|value| value != 0)
        .map_err(|_| FinalizerError::Decode("bad stack num"))
}

fn extract_stack_string(value: &Value) -> Option<&str> {
    match value {
        Value::String(value) => Some(value.as_str()),
        Value::Array(values) => values.get(1).and_then(extract_stack_string),
        Value::Object(map) => map
            .get("value")
            .or_else(|| map.get("cell"))
            .or_else(|| map.get("boc"))
            .or_else(|| map.get("bytes"))
            .and_then(extract_stack_string),
        _ => None,
    }
}

fn parse_hash_or_base64(value: &str) -> Result<Hash32, FinalizerError> {
    let value = value.trim();
    if value.starts_with("0x")
        || value.len() == 64 && value.chars().all(|char| char.is_ascii_hexdigit())
    {
        return Hash32::from_hex(value).map_err(|_| FinalizerError::Decode("bad hash hex"));
    }
    let decoded = BASE64_STANDARD
        .decode(value)
        .or_else(|_| BASE64_URL_SAFE_NO_PAD.decode(value))
        .map_err(|_| FinalizerError::Decode("bad hash encoding"))?;
    let bytes: [u8; 32] = decoded
        .try_into()
        .map_err(|_| FinalizerError::Decode("hash must be 32 bytes"))?;
    Ok(Hash32::new(bytes))
}
