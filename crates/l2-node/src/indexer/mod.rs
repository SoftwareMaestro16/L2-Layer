use crate::config::{NodeConfig, SecretString};
use crate::storage::{DynStorage, L1Cursor, StorageError};
use async_trait::async_trait;
use l2_core::crypto::hash_domain;
use l2_core::{DepositEvent, Hash32, Sequencer};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tonlib_core::cell::BagOfCells;

mod parse;

#[cfg(test)]
use parse::parse_hash_or_base64;
use parse::{
    biguint_to_u128, field, hash32_from_tonhash, parse_message_hash, parse_opcode,
    parse_u128_value, parse_u32_value, parse_u64_value, parse_uint256_hash, ton_addresses_match,
};

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

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
