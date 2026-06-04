use crate::batch::{BatchBuildInput, BatchBuilder};
use crate::crypto::{derive_account_id, verify_signature, Hash32};
use crate::executor::{DeterministicExecutor, ExecutionConfig};
use crate::gas::GasSchedule;
use crate::state::State;
use crate::types::{
    DepositEvent, L2Block, L2BlockHeader, L2TransactionKind, Receipt, SignedL2Transaction,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SequencerConfig {
    pub chain_id: String,
    pub max_txs_per_block: usize,
    pub block_gas_limit: u64,
    pub gas_coin_asset: u32,
    pub gas_schedule: GasSchedule,
    pub max_internal_messages: u32,
}

impl Default for SequencerConfig {
    fn default() -> Self {
        Self {
            chain_id: "ton-l2-devnet".to_owned(),
            max_txs_per_block: 1024,
            block_gas_limit: 1_000_000,
            gas_coin_asset: crate::types::L2_NATIVE_GAS_ASSET,
            gas_schedule: GasSchedule::default(),
            max_internal_messages: 1024,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Mempool {
    pending: VecDeque<QueuedTransaction>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransactionOrigin {
    User,
    System,
}

#[derive(Clone, Debug)]
struct QueuedTransaction {
    tx: SignedL2Transaction,
    origin: TransactionOrigin,
}

impl Mempool {
    pub fn submit(&mut self, tx: SignedL2Transaction) -> Hash32 {
        let hash = tx.tx_hash();
        self.pending.push_back(QueuedTransaction {
            tx,
            origin: TransactionOrigin::User,
        });
        hash
    }

    pub fn insert_system_deposits(&mut self, chain_id: &str, deposits: Vec<DepositEvent>) {
        for deposit in deposits {
            self.pending.push_back(
                SignedL2Transaction::system_deposit(
                    chain_id,
                    deposit.deposit_id,
                    deposit.asset_id,
                    deposit.recipient,
                    deposit.amount,
                )
                .into_system_queue_entry(),
            );
        }
    }

    fn select_block(&mut self, max_txs: usize) -> Vec<QueuedTransaction> {
        let mut out = Vec::with_capacity(max_txs.min(self.pending.len()));
        for _ in 0..max_txs {
            let Some(tx) = self.pending.pop_front() else {
                break;
            };
            out.push(tx);
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

trait IntoSystemQueueEntry {
    fn into_system_queue_entry(self) -> QueuedTransaction;
}

impl IntoSystemQueueEntry for SignedL2Transaction {
    fn into_system_queue_entry(self) -> QueuedTransaction {
        QueuedTransaction {
            tx: self,
            origin: TransactionOrigin::System,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Sequencer {
    pub config: SequencerConfig,
    pub state: State,
    pub mempool: Mempool,
    executor: DeterministicExecutor,
    committed_deposits: BTreeSet<Hash32>,
    last_header: Option<L2BlockHeader>,
}

impl Sequencer {
    pub fn new(config: SequencerConfig) -> Self {
        Self {
            config,
            state: State::default(),
            mempool: Mempool::default(),
            executor: DeterministicExecutor,
            committed_deposits: BTreeSet::new(),
            last_header: None,
        }
    }

    pub fn submit_tx(&mut self, tx: SignedL2Transaction) -> Hash32 {
        self.mempool.submit(tx)
    }

    pub fn ingest_deposits(&mut self, deposits: Vec<DepositEvent>) {
        let new_deposits = deposits
            .into_iter()
            .filter(|deposit| self.committed_deposits.insert(deposit.deposit_id))
            .collect::<Vec<_>>();
        self.mempool
            .insert_system_deposits(&self.config.chain_id, new_deposits);
    }

    pub fn produce_block(&mut self, timestamp: u64) -> Option<L2Block> {
        if self.mempool.is_empty() {
            return None;
        }

        let block_height = self.next_height();
        let prev_state_root = self.state.root_hash();
        let queued_txs = self.mempool.select_block(self.config.max_txs_per_block);
        let mut receipts = Vec::with_capacity(queued_txs.len());
        let mut withdrawals = Vec::new();
        let mut block_gas_used = 0u64;

        for queued in &queued_txs {
            if let Err(reason) = self.verify_tx(&queued.tx, queued.origin) {
                receipts.push(Receipt::rejected(queued.tx.tx_hash(), reason));
                continue;
            }

            let required_gas = self.config.gas_schedule.required_gas(&queued.tx.kind);
            let Some(next_block_gas_used) = block_gas_used.checked_add(required_gas) else {
                receipts.push(Receipt::rejected(
                    queued.tx.tx_hash(),
                    "block_gas_limit_exceeded",
                ));
                continue;
            };
            if next_block_gas_used > self.config.block_gas_limit {
                receipts.push(Receipt::rejected(
                    queued.tx.tx_hash(),
                    "block_gas_limit_exceeded",
                ));
                continue;
            }

            let outcome = self.executor.apply(
                &mut self.state,
                &queued.tx,
                &ExecutionConfig {
                    block_time: timestamp,
                    block_height,
                    gas_coin_asset: self.config.gas_coin_asset,
                    gas_schedule: self.config.gas_schedule,
                    max_internal_messages: self.config.max_internal_messages,
                },
            );
            block_gas_used = next_block_gas_used;
            receipts.push(outcome.receipt);
            withdrawals.extend(outcome.withdrawals);
        }

        let state_root = self.state.root_hash();
        let txs = queued_txs
            .into_iter()
            .map(|queued| queued.tx)
            .collect::<Vec<_>>();
        let block = BatchBuilder::build(BatchBuildInput {
            previous_header: self.last_header.clone(),
            prev_state_root,
            state_root,
            ordered_transactions: txs,
            receipts,
            withdrawals,
            timestamp,
        })
        .expect("sequencer emits exactly one receipt per selected transaction");

        self.last_header = Some(block.header.clone());
        Some(block)
    }

    fn next_height(&self) -> u64 {
        self.last_header
            .as_ref()
            .map_or(0, |header| header.height + 1)
    }

    fn verify_tx(
        &self,
        tx: &SignedL2Transaction,
        origin: TransactionOrigin,
    ) -> Result<(), &'static str> {
        if tx.chain_id != self.config.chain_id {
            return Err("wrong_chain_id");
        }

        if origin == TransactionOrigin::System {
            if !tx.is_system() {
                return Err("system_tx_must_be_deposit");
            }
            if tx.from.is_some() || tx.public_key.is_some() || tx.signature.is_some() {
                return Err("invalid_system_tx_auth");
            }
            return Ok(());
        }

        if tx.is_system() {
            return Err("deposit_must_be_system");
        }

        let from = tx.from.ok_or("missing_sender")?;
        let public_key_hex = tx.public_key.as_deref().ok_or("missing_public_key")?;
        let signature_hex = tx.signature.as_deref().ok_or("missing_signature")?;
        let public_key =
            crate::crypto::decode_fixed::<32>(public_key_hex).map_err(|_| "invalid_public_key")?;
        if derive_account_id(&public_key) != from {
            return Err("public_key_sender_mismatch");
        }
        if !verify_signature(public_key_hex, signature_hex, &tx.signing_payload()) {
            return Err("bad_signature");
        }

        let account = self.state.account(from).ok_or("unknown_sender")?;
        if account.nonce != tx.nonce {
            return Err("bad_nonce");
        }

        match tx.kind {
            L2TransactionKind::Deposit { .. } => Err("deposit_must_be_system"),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "sequencer_tests.rs"]
mod tests;
