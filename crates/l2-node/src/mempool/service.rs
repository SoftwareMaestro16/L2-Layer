use base64::prelude::{Engine as _, BASE64_STANDARD};
use l2_core::address::is_l2_zero_address;
use l2_core::crypto::{decode_public_key, verify_signature, Hash32};
use l2_core::{
    L2TransactionKind, SignedL2Transaction, L2_NATIVE_GAS_ASSET, L2_TRANSACTION_KIND_VERSION_V1,
    L2_TX_DOMAIN_SEPARATOR, L2_TX_VERSION_V2,
};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::NodeConfig;

use super::config::MempoolAdmissionConfig;
use super::error::MempoolError;
use super::redis_store::RedisMempoolStore;
use super::types::{
    DynMempoolStore, MempoolCounters, MempoolMetrics, MempoolPayloadClass, MempoolStoreLimits,
    MempoolTxPriority, ValidatedMempoolTx,
};

const DEFAULT_REDIS_PREFIX: &str = "entropis:testnet";

#[derive(Clone)]
pub struct MempoolService {
    chain_id: String,
    config: MempoolAdmissionConfig,
    store: DynMempoolStore,
    counters: Arc<Mutex<MempoolCounters>>,
}

impl MempoolService {
    pub fn new(chain_id: impl Into<String>, store: DynMempoolStore) -> Self {
        Self::with_config(chain_id, MempoolAdmissionConfig::default(), store)
    }

    pub fn with_config(
        chain_id: impl Into<String>,
        config: MempoolAdmissionConfig,
        store: DynMempoolStore,
    ) -> Self {
        Self {
            chain_id: chain_id.into(),
            config,
            store,
            counters: Arc::new(Mutex::new(MempoolCounters::default())),
        }
    }

    pub async fn submit(&self, tx: SignedL2Transaction) -> Result<Hash32, MempoolError> {
        match self.submit_inner(tx).await {
            Ok(tx_hash) => {
                self.record_accepted().await;
                Ok(tx_hash)
            }
            Err(error) => {
                self.record_rejection(error.reason_code()).await;
                Err(error)
            }
        }
    }

    async fn submit_inner(&self, tx: SignedL2Transaction) -> Result<Hash32, MempoolError> {
        let validated = self.prevalidate_public_tx(tx)?;
        self.store
            .consume_rate_limit(
                validated.account_id,
                self.config.rate_limit_window,
                self.config.max_account_submissions_per_window,
            )
            .await?;

        let public_key_hex = validated
            .tx
            .public_key
            .as_deref()
            .ok_or(MempoolError::MissingPublicKey)?;
        let signature_hex = validated
            .tx
            .signature
            .as_deref()
            .ok_or(MempoolError::MissingSignature)?;
        if !verify_signature(
            public_key_hex,
            signature_hex,
            &validated.tx.signing_payload(),
        ) {
            return Err(MempoolError::BadSignature);
        }

        let tx_hash = validated.tx_hash;
        self.store
            .enqueue_validated(
                validated,
                MempoolStoreLimits {
                    replay_ttl: self.config.replay_ttl,
                    nonce_lock_ttl: self.config.nonce_lock_ttl,
                    max_global_queue: self.config.max_global_queue,
                    max_account_queue: self.config.max_account_queue,
                    max_account_nonce_window: self.config.max_account_nonce_window,
                },
            )
            .await?;
        Ok(tx_hash)
    }

    pub async fn pop_batch(
        &self,
        max_txs: usize,
    ) -> Result<Vec<SignedL2Transaction>, MempoolError> {
        self.store.pop_batch(max_txs).await
    }

    pub async fn acquire_leader_lock(&self, owner: &str) -> Result<bool, MempoolError> {
        self.store
            .acquire_leader_lock(owner, self.config.leader_ttl)
            .await
    }

    pub async fn release_leader_lock(&self, owner: &str) -> Result<bool, MempoolError> {
        self.store.release_leader_lock(owner).await
    }

    pub async fn metrics(&self) -> Result<MempoolMetrics, MempoolError> {
        let counters = self.counters.lock().await;
        Ok(MempoolMetrics {
            accepted: counters.accepted,
            rejected: counters.rejected.clone(),
            store: self.store.stats().await?,
        })
    }

    pub async fn health_check(&self) -> Result<(), MempoolError> {
        self.store.stats().await.map(|_| ())
    }

    fn prevalidate_public_tx(
        &self,
        tx: SignedL2Transaction,
    ) -> Result<ValidatedMempoolTx, MempoolError> {
        if tx.chain_id != self.chain_id {
            return Err(MempoolError::WrongChainId);
        }
        if matches!(
            tx.kind,
            L2TransactionKind::Deposit { .. } | L2TransactionKind::InternalMessage { .. }
        ) {
            return Err(MempoolError::SystemTxNotAllowed);
        }
        self.validate_envelope(&tx)?;
        self.validate_admission_policy(&tx)?;

        let from = tx.from.ok_or(MempoolError::MissingSender)?;
        if is_l2_zero_address(from) {
            return Err(MempoolError::ReservedZeroAddress);
        }
        if self.config.banned_accounts.contains(&from) {
            return Err(MempoolError::AccountBanned { account_id: from });
        }
        let public_key_hex = tx
            .public_key
            .as_deref()
            .ok_or(MempoolError::MissingPublicKey)?;
        let _signature_hex = tx
            .signature
            .as_deref()
            .ok_or(MempoolError::MissingSignature)?;
        decode_public_key(public_key_hex).map_err(|_| MempoolError::InvalidPublicKey)?;

        Ok(ValidatedMempoolTx {
            tx_hash: tx.tx_hash(),
            nonce: tx.nonce,
            account_id: from,
            priority: MempoolTxPriority::from_tx(&tx),
            tx,
        })
    }

    fn validate_envelope(&self, tx: &SignedL2Transaction) -> Result<(), MempoolError> {
        if tx.tx_version != L2_TX_VERSION_V2 {
            return Err(MempoolError::UnsupportedTxVersion);
        }
        if tx.domain_separator != L2_TX_DOMAIN_SEPARATOR {
            return Err(MempoolError::InvalidDomainSeparator);
        }
        if tx.transaction_kind_version != L2_TRANSACTION_KIND_VERSION_V1 {
            return Err(MempoolError::UnsupportedTransactionKindVersion);
        }
        if tx.fee_asset_id != L2_NATIVE_GAS_ASSET {
            return Err(MempoolError::UnsupportedFeeAsset {
                asset_id: tx.fee_asset_id,
            });
        }
        Ok(())
    }

    fn validate_admission_policy(&self, tx: &SignedL2Transaction) -> Result<(), MempoolError> {
        let payload_bytes = serde_json::to_vec(tx)?.len();
        if payload_bytes > self.config.max_payload_bytes {
            return Err(MempoolError::PayloadTooLarge {
                bytes: payload_bytes,
                max: self.config.max_payload_bytes,
            });
        }
        if let Some(class) = MempoolPayloadClass::from_kind(&tx.kind) {
            let max = self.payload_class_limit(class);
            if payload_bytes > max {
                return Err(MempoolError::PayloadClassTooLarge {
                    class: class.limit_name(),
                    reason_code: class.reason_code(),
                    bytes: payload_bytes,
                    max,
                });
            }
        }
        if tx.gas_limit < self.config.min_gas_limit || tx.gas_limit > self.config.max_gas_limit {
            return Err(MempoolError::InvalidGasLimit {
                gas_limit: tx.gas_limit,
                min: self.config.min_gas_limit,
                max: self.config.max_gas_limit,
            });
        }
        if tx.max_gas_price < self.config.min_gas_price {
            return Err(MempoolError::GasPriceTooLow {
                gas_price: tx.max_gas_price,
                min: self.config.min_gas_price,
            });
        }
        let max_fee = u128::from(tx.gas_limit)
            .checked_mul(tx.max_gas_price)
            .ok_or(MempoolError::TxFeeOverflow)?;
        if max_fee > self.config.max_tx_fee {
            return Err(MempoolError::TxFeeTooHigh {
                fee: max_fee,
                max: self.config.max_tx_fee,
            });
        }
        match &tx.kind {
            L2TransactionKind::Transfer { to, .. } if is_l2_zero_address(*to) => {
                return Err(MempoolError::ReservedZeroAddress);
            }
            L2TransactionKind::CallContract {
                contract,
                body_boc_base64,
            } => {
                if is_l2_zero_address(*contract) {
                    return Err(MempoolError::ReservedZeroAddress);
                }
                if body_boc_base64.len() > self.config.max_call_body_boc_base64_bytes {
                    return Err(MempoolError::CallBodyTooLarge {
                        bytes: body_boc_base64.len(),
                        max: self.config.max_call_body_boc_base64_bytes,
                    });
                }
                BASE64_STANDARD
                    .decode(body_boc_base64.as_bytes())
                    .map_err(|_| MempoolError::BadCallBodyBase64)?;
            }
            L2TransactionKind::DeployContract {
                contract,
                code_boc_base64,
                data_boc_base64,
            } => {
                if is_l2_zero_address(*contract) {
                    return Err(MempoolError::ReservedZeroAddress);
                }
                self.validate_deploy_boc("code_boc_base64", code_boc_base64)?;
                self.validate_deploy_boc("data_boc_base64", data_boc_base64)?;
            }
            L2TransactionKind::RotatePublicKey { new_public_key } => {
                decode_public_key(new_public_key).map_err(|_| MempoolError::InvalidPublicKey)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn payload_class_limit(&self, class: MempoolPayloadClass) -> usize {
        match class {
            MempoolPayloadClass::Transfer => self.config.max_transfer_payload_bytes,
            MempoolPayloadClass::Withdraw => self.config.max_withdraw_payload_bytes,
            MempoolPayloadClass::CallContract => self.config.max_call_payload_bytes,
            MempoolPayloadClass::DeployContract => self.config.max_deploy_payload_bytes,
            MempoolPayloadClass::RotatePublicKey => self.config.max_payload_bytes,
        }
    }

    fn validate_deploy_boc(&self, field: &'static str, value: &str) -> Result<(), MempoolError> {
        if value.len() > self.config.max_call_body_boc_base64_bytes {
            return Err(MempoolError::DeployBocTooLarge {
                field,
                bytes: value.len(),
                max: self.config.max_call_body_boc_base64_bytes,
            });
        }
        BASE64_STANDARD
            .decode(value.as_bytes())
            .map_err(|_| MempoolError::BadDeployBocBase64 { field })?;
        Ok(())
    }

    async fn record_accepted(&self) {
        self.counters.lock().await.accepted += 1;
    }

    async fn record_rejection(&self, reason: &str) {
        let mut counters = self.counters.lock().await;
        *counters.rejected.entry(reason.to_owned()).or_default() += 1;
    }

    pub async fn record_external_rejection(&self, reason: &'static str) {
        self.record_rejection(reason).await;
    }
}

pub async fn build_mempool(config: &NodeConfig) -> Result<MempoolService, MempoolError> {
    let store = RedisMempoolStore::connect(config.redis_url.expose(), DEFAULT_REDIS_PREFIX).await?;
    Ok(MempoolService::with_config(
        config.chain_id.clone(),
        MempoolAdmissionConfig::from_config(config),
        Arc::new(store),
    ))
}
