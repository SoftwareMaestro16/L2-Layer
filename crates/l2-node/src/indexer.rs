use crate::config::{NodeConfig, SecretString};
use crate::storage::{DynStorage, L1Cursor, StorageError};
use async_trait::async_trait;
use base64::prelude::{Engine as _, BASE64_STANDARD, BASE64_URL_SAFE_NO_PAD};
use l2_core::crypto::hash_domain;
use l2_core::{DepositEvent, Hash32, Sequencer};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tonlib_core::cell::BagOfCells;
use tonlib_core::types::TonAddress;

const DEPOSIT_RECORDED_OPCODE: u32 = 0x4c324407;
const CURSOR_SOURCE: &str = "toncenter:vault-deposits";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepositIndexerConfig {
    pub vault_address: String,
    pub allowed_asset_ids: Vec<u32>,
    pub batch_limit: u16,
    pub confirmation_lag_lt: u64,
}

impl DepositIndexerConfig {
    pub fn from_node_config(config: &NodeConfig) -> Option<Self> {
        config.l1_deposit_indexer_enabled.then(|| Self {
            vault_address: config
                .l1_vault_address
                .clone()
                .expect("validated indexer config has vault address"),
            allowed_asset_ids: config.l1_deposit_asset_ids.clone(),
            batch_limit: config.l1_deposit_batch_limit,
            confirmation_lag_lt: config.l1_deposit_confirmation_lag_lt,
        })
    }

    pub fn cursor_source(&self) -> String {
        format!("{CURSOR_SOURCE}:{}", self.vault_address)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToncenterMessagesRequest {
    pub source: String,
    pub start_lt: u64,
    pub limit: u16,
    pub opcode: u32,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IndexerPollStats {
    pub fetched: usize,
    pub accepted: usize,
    pub duplicates: usize,
}

#[async_trait]
pub trait TonMessageClient: Send + Sync {
    async fn get_deposit_logs(
        &self,
        request: ToncenterMessagesRequest,
    ) -> Result<Vec<ToncenterMessage>, IndexerError>;
}

#[derive(Clone, Debug)]
pub struct ToncenterClient {
    base_url: String,
    api_key: SecretString,
    client: reqwest::Client,
}

impl ToncenterClient {
    pub fn from_config(config: &NodeConfig) -> Self {
        Self {
            base_url: config
                .toncenter_v3_base_url
                .trim_end_matches('/')
                .to_owned(),
            api_key: config.toncenter_api_key.clone(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TonMessageClient for ToncenterClient {
    async fn get_deposit_logs(
        &self,
        request: ToncenterMessagesRequest,
    ) -> Result<Vec<ToncenterMessage>, IndexerError> {
        let opcode = format!("0x{:08x}", request.opcode);
        let start_lt = request.start_lt.to_string();
        let limit = request.limit.to_string();
        let response = self
            .client
            .get(format!("{}/messages", self.base_url))
            .header("X-API-Key", self.api_key.expose())
            .query(&[
                ("source", request.source.as_str()),
                ("destination", "null"),
                ("opcode", opcode.as_str()),
                ("start_lt", start_lt.as_str()),
                ("limit", limit.as_str()),
                ("sort", "asc"),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<ToncenterMessagesResponse>()
            .await?;
        Ok(response.messages)
    }
}

#[derive(Clone, Debug)]
pub struct TonDepositIndexer<C> {
    config: DepositIndexerConfig,
    client: C,
}

impl<C> TonDepositIndexer<C> {
    pub fn new(config: DepositIndexerConfig, client: C) -> Self {
        Self { config, client }
    }
}

impl<C> TonDepositIndexer<C>
where
    C: TonMessageClient,
{
    pub async fn poll_once(
        &self,
        storage: &DynStorage,
        sequencer: &Arc<RwLock<Sequencer>>,
    ) -> Result<IndexerPollStats, IndexerError> {
        let cursor_source = self.config.cursor_source();
        let cursor = storage.get_l1_cursor(&cursor_source).await?;
        let start_lt = cursor
            .as_ref()
            .and_then(|cursor| cursor.lt.checked_add(1))
            .unwrap_or(1);
        let messages = self
            .client
            .get_deposit_logs(ToncenterMessagesRequest {
                source: self.config.vault_address.clone(),
                start_lt,
                limit: self.config.batch_limit,
                opcode: DEPOSIT_RECORDED_OPCODE,
            })
            .await?;

        let mut stats = IndexerPollStats {
            fetched: messages.len(),
            ..IndexerPollStats::default()
        };
        let max_seen_lt = messages
            .iter()
            .filter_map(|message| parse_u64_value(message.created_lt.as_ref(), "created_lt").ok())
            .max()
            .unwrap_or_default();
        for message in messages {
            let message_lt = parse_u64_value(message.created_lt.as_ref(), "created_lt")?;
            if message_lt.saturating_add(self.config.confirmation_lag_lt) > max_seen_lt {
                break;
            }
            let deposit = parse_deposit_message(&message, &self.config)?;
            if let Some(cursor) = cursor.as_ref() {
                if deposit.l1_lt < cursor.lt {
                    return Err(IndexerError::Validation("message lt moved backwards"));
                }
            }
            let inserted = storage.save_deposit(deposit.clone()).await?;
            if inserted {
                sequencer
                    .write()
                    .await
                    .ingest_deposits(vec![deposit.clone()]);
                stats.accepted += 1;
            } else {
                stats.duplicates += 1;
            }
            storage
                .set_l1_cursor(
                    &cursor_source,
                    L1Cursor {
                        lt: deposit.l1_lt,
                        hash: deposit.l1_tx_hash,
                    },
                )
                .await?;
        }
        Ok(stats)
    }
}

pub fn parse_deposit_message(
    message: &ToncenterMessage,
    config: &DepositIndexerConfig,
) -> Result<DepositEvent, IndexerError> {
    let source = message
        .source
        .as_deref()
        .ok_or(IndexerError::Validation("deposit log source is missing"))?;
    if !ton_addresses_match(source, &config.vault_address) {
        return Err(IndexerError::Validation("deposit log source is not vault"));
    }
    if message
        .destination
        .as_deref()
        .is_some_and(|value| value != "null")
    {
        return Err(IndexerError::Validation(
            "deposit log is not an external log",
        ));
    }
    if parse_opcode(message.opcode.as_ref())? != DEPOSIT_RECORDED_OPCODE {
        return Err(IndexerError::Validation("unexpected deposit event opcode"));
    }

    let l1_lt = parse_u64_value(message.created_lt.as_ref(), "created_lt")?;
    if l1_lt == 0 {
        return Err(IndexerError::Validation("created_lt must be non-zero"));
    }
    let l1_tx_hash = parse_message_hash(message.hash_norm.as_ref().or(message.hash.as_ref()))?;
    if l1_tx_hash == Hash32::ZERO {
        return Err(IndexerError::Validation("message hash must be non-zero"));
    }

    let decoded = decode_deposit_recorded(message)?;

    if decoded.event_deposit_id == Hash32::ZERO {
        return Err(IndexerError::Validation("deposit id must be non-zero"));
    }
    if !config.allowed_asset_ids.contains(&decoded.asset_id) {
        return Err(IndexerError::Validation("unexpected deposit asset id"));
    }
    if decoded.amount == 0 {
        return Err(IndexerError::Validation("deposit amount must be non-zero"));
    }
    if decoded.recipient == Hash32::ZERO {
        return Err(IndexerError::Validation(
            "deposit recipient must be non-zero",
        ));
    }

    Ok(DepositEvent {
        deposit_id: canonical_deposit_id(source, l1_tx_hash, l1_lt, decoded.event_deposit_id),
        asset_id: decoded.asset_id,
        recipient: decoded.recipient,
        amount: decoded.amount,
        l1_tx_hash,
        l1_lt,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedDepositRecorded {
    event_deposit_id: Hash32,
    asset_id: u32,
    amount: u128,
    recipient: Hash32,
    query_id: u64,
}

fn decode_deposit_recorded(
    message: &ToncenterMessage,
) -> Result<DecodedDepositRecorded, IndexerError> {
    let content = message
        .message_content
        .as_ref()
        .ok_or(IndexerError::Decode("message_content is missing"))?;
    if let Some(decoded) = content.decoded.as_ref() {
        return decode_deposit_recorded_json(decoded);
    }
    let body = content
        .body
        .as_deref()
        .ok_or(IndexerError::Decode("message_content body is missing"))?;
    decode_deposit_recorded_boc(body)
}

fn decode_deposit_recorded_json(decoded: &Value) -> Result<DecodedDepositRecorded, IndexerError> {
    let event_deposit_id = parse_uint256_hash(field(decoded, &["depositId", "deposit_id"])?)?;
    let asset_id = parse_u32_value(field(decoded, &["assetId", "asset_id"])?, "asset_id")?;
    let amount = parse_u128_value(field(decoded, &["amount"])?, "amount")?;
    let recipient = parse_uint256_hash(field(decoded, &["l2Recipient", "l2_recipient"])?)?;
    let query_id = parse_u64_value(Some(field(decoded, &["queryId", "query_id"])?), "query_id")?;

    Ok(DecodedDepositRecorded {
        event_deposit_id,
        asset_id,
        amount,
        recipient,
        query_id,
    })
}

fn decode_deposit_recorded_boc(
    body_boc_base64: &str,
) -> Result<DecodedDepositRecorded, IndexerError> {
    let root = BagOfCells::parse_base64(body_boc_base64)
        .and_then(BagOfCells::single_root)
        .map_err(|_| IndexerError::Decode("message_content body BoC is malformed"))?;
    let mut parser = root.parser();
    let opcode = parser
        .load_u32(32)
        .map_err(|_| IndexerError::Decode("body opcode is missing"))?;
    if opcode != DEPOSIT_RECORDED_OPCODE {
        return Err(IndexerError::Validation("unexpected deposit event opcode"));
    }
    let query_id = parser
        .load_u64(64)
        .map_err(|_| IndexerError::Decode("query_id is malformed"))?;
    let event_deposit_id = parser
        .load_tonhash()
        .map(hash32_from_tonhash)
        .map_err(|_| IndexerError::Decode("deposit_id is malformed"))?;
    let asset_id = parser
        .load_u32(32)
        .map_err(|_| IndexerError::Decode("asset_id is malformed"))?;
    let amount = parser
        .load_coins()
        .map_err(|_| IndexerError::Decode("amount is malformed"))
        .and_then(biguint_to_u128)?;
    let recipient = parser
        .load_tonhash()
        .map(hash32_from_tonhash)
        .map_err(|_| IndexerError::Decode("l2_recipient is malformed"))?;
    parser
        .next_reference()
        .map_err(|_| IndexerError::Decode("extra ref is missing"))?;
    if parser.remaining_bits() != 0 || parser.remaining_refs() != 0 {
        return Err(IndexerError::Decode("deposit body has trailing data"));
    }

    Ok(DecodedDepositRecorded {
        event_deposit_id,
        asset_id,
        amount,
        recipient,
        query_id,
    })
}

fn canonical_deposit_id(
    source: &str,
    l1_tx_hash: Hash32,
    l1_lt: u64,
    event_deposit_id: Hash32,
) -> Hash32 {
    hash_domain(
        "entropis.l1.deposit.event.v1",
        &[
            source.as_bytes(),
            l1_tx_hash.as_bytes(),
            &l1_lt.to_be_bytes(),
            event_deposit_id.as_bytes(),
        ],
    )
}

#[derive(Clone, Debug, Deserialize)]
pub struct ToncenterMessagesResponse {
    #[serde(default)]
    pub messages: Vec<ToncenterMessage>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ToncenterMessage {
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub hash_norm: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub destination: Option<String>,
    #[serde(default)]
    pub opcode: Option<Value>,
    #[serde(default)]
    pub created_lt: Option<Value>,
    #[serde(default)]
    pub message_content: Option<ToncenterMessageContent>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ToncenterMessageContent {
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub decoded: Option<Value>,
}

#[derive(Debug, Error)]
pub enum IndexerError {
    #[error("toncenter request failed")]
    Http(#[from] reqwest::Error),
    #[error("ton deposit event decoding failed: {0}")]
    Decode(&'static str),
    #[error("ton deposit event validation failed: {0}")]
    Validation(&'static str),
    #[error("storage failed: {0}")]
    Storage(#[from] StorageError),
}

fn parse_opcode(value: Option<&Value>) -> Result<u32, IndexerError> {
    match value {
        Some(Value::Number(number)) => number
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(IndexerError::Decode("opcode is not uint32")),
        Some(Value::String(value)) => {
            let value = value.trim();
            if let Some(hex) = value.strip_prefix("0x") {
                u32::from_str_radix(hex, 16).map_err(|_| IndexerError::Decode("bad opcode hex"))
            } else {
                value
                    .parse::<u32>()
                    .map_err(|_| IndexerError::Decode("bad opcode"))
            }
        }
        _ => Err(IndexerError::Decode("opcode is missing")),
    }
}

fn field<'a>(value: &'a Value, names: &[&str]) -> Result<&'a Value, IndexerError> {
    let object = value
        .as_object()
        .ok_or(IndexerError::Decode("decoded payload is not an object"))?;
    names
        .iter()
        .find_map(|name| object.get(*name))
        .ok_or(IndexerError::Decode("decoded payload field is missing"))
}

fn parse_u64_value(value: Option<&Value>, field: &'static str) -> Result<u64, IndexerError> {
    let value = value.ok_or(IndexerError::Decode(field))?;
    match value {
        Value::Number(number) => number.as_u64().ok_or(IndexerError::Decode(field)),
        Value::String(value) => value
            .parse::<u64>()
            .map_err(|_| IndexerError::Decode(field)),
        _ => Err(IndexerError::Decode(field)),
    }
}

fn parse_u32_value(value: &Value, field: &'static str) -> Result<u32, IndexerError> {
    let value = parse_u64_value(Some(value), field)?;
    u32::try_from(value).map_err(|_| IndexerError::Decode(field))
}

fn parse_u128_value(value: &Value, field: &'static str) -> Result<u128, IndexerError> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .map(u128::from)
            .ok_or(IndexerError::Decode(field)),
        Value::String(value) => value
            .parse::<u128>()
            .map_err(|_| IndexerError::Decode(field)),
        _ => Err(IndexerError::Decode(field)),
    }
}

fn parse_uint256_hash(value: &Value) -> Result<Hash32, IndexerError> {
    match value {
        Value::String(value) => parse_hash_or_decimal(value),
        Value::Number(number) => {
            let value = number
                .as_u64()
                .ok_or(IndexerError::Decode("uint256 number is invalid"))?;
            Ok(uint256_decimal_to_hash(&value.to_string())?)
        }
        _ => Err(IndexerError::Decode("uint256 field is invalid")),
    }
}

fn parse_message_hash(value: Option<&String>) -> Result<Hash32, IndexerError> {
    let value = value.ok_or(IndexerError::Decode("message hash is missing"))?;
    parse_hash_or_base64(value)
}

fn ton_addresses_match(left: &str, right: &str) -> bool {
    match (canonical_ton_address(left), canonical_ton_address(right)) {
        (Some(left), Some(right)) => left == right,
        _ => left == right,
    }
}

fn canonical_ton_address(value: &str) -> Option<String> {
    TonAddress::from_base64_url(value)
        .or_else(|_| TonAddress::from_base64_std(value))
        .or_else(|_| TonAddress::from_hex_str(value))
        .ok()
        .map(|address| address.to_hex())
}

fn hash32_from_tonhash(value: tonlib_core::types::TonHash) -> Hash32 {
    let bytes: [u8; 32] = value
        .as_slice()
        .try_into()
        .expect("TonHash is always 32 bytes");
    Hash32::new(bytes)
}

fn biguint_to_u128(value: num_bigint::BigUint) -> Result<u128, IndexerError> {
    let bytes = value.to_bytes_be();
    if bytes.len() > 16 {
        return Err(IndexerError::Decode("amount exceeds u128"));
    }
    let mut out = [0u8; 16];
    out[16 - bytes.len()..].copy_from_slice(&bytes);
    Ok(u128::from_be_bytes(out))
}

fn parse_hash_or_decimal(value: &str) -> Result<Hash32, IndexerError> {
    let value = value.trim();
    if value.starts_with("0x")
        || value.len() == 64 && value.chars().all(|char| char.is_ascii_hexdigit())
    {
        return Hash32::from_hex(value).map_err(|_| IndexerError::Decode("bad uint256 hex"));
    }
    uint256_decimal_to_hash(value)
}

fn parse_hash_or_base64(value: &str) -> Result<Hash32, IndexerError> {
    let value = value.trim();
    if value.starts_with("0x")
        || value.len() == 64 && value.chars().all(|char| char.is_ascii_hexdigit())
    {
        return Hash32::from_hex(value).map_err(|_| IndexerError::Decode("bad hash hex"));
    }

    let decoded = BASE64_STANDARD
        .decode(value)
        .or_else(|_| BASE64_URL_SAFE_NO_PAD.decode(value))
        .map_err(|_| IndexerError::Decode("bad hash encoding"))?;
    let bytes: [u8; 32] = decoded
        .try_into()
        .map_err(|_| IndexerError::Decode("hash must be 32 bytes"))?;
    Ok(Hash32::new(bytes))
}

fn uint256_decimal_to_hash(value: &str) -> Result<Hash32, IndexerError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(IndexerError::Decode("bad uint256 decimal"));
    }
    let mut out = [0u8; 32];
    for digit in value.bytes().map(|byte| byte - b'0') {
        let mut carry = u16::from(digit);
        for byte in out.iter_mut().rev() {
            let next = u16::from(*byte) * 10 + carry;
            *byte = next as u8;
            carry = next >> 8;
        }
        if carry != 0 {
            return Err(IndexerError::Decode("uint256 decimal overflow"));
        }
    }
    Ok(Hash32::new(out))
}

#[cfg(test)]
#[path = "indexer_tests.rs"]
mod tests;
