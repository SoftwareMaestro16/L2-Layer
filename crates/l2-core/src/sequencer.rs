use crate::crypto::{derive_account_id, hash_domain, verify_signature, Hash32};
use crate::executor::{DeterministicExecutor, ExecutionConfig};
use crate::state::State;
use crate::types::{DepositEvent, L2Block, L2TransactionKind, Receipt, SignedL2Transaction};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SequencerConfig {
    pub chain_id: String,
    pub max_txs_per_block: usize,
    pub block_gas_limit: u64,
    pub gas_coin_asset: u32,
    pub max_internal_messages: u32,
}

impl Default for SequencerConfig {
    fn default() -> Self {
        Self {
            chain_id: "ton-l2-devnet".to_owned(),
            max_txs_per_block: 1024,
            block_gas_limit: 1_000_000,
            gas_coin_asset: crate::types::L2_NATIVE_GAS_ASSET,
            max_internal_messages: 1024,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Mempool {
    pending: VecDeque<SignedL2Transaction>,
}

impl Mempool {
    pub fn submit(&mut self, tx: SignedL2Transaction) -> Hash32 {
        let hash = tx.tx_hash();
        self.pending.push_back(tx);
        hash
    }

    pub fn insert_system_deposits(&mut self, chain_id: &str, deposits: Vec<DepositEvent>) {
        for deposit in deposits {
            self.pending.push_back(SignedL2Transaction::system_deposit(
                chain_id,
                deposit.deposit_id,
                deposit.asset_id,
                deposit.recipient,
                deposit.amount,
            ));
        }
    }

    pub fn select_block(&mut self, max_txs: usize) -> Vec<SignedL2Transaction> {
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

#[derive(Clone, Debug)]
pub struct Sequencer {
    pub config: SequencerConfig,
    pub state: State,
    pub mempool: Mempool,
    executor: DeterministicExecutor,
    committed_deposits: BTreeSet<Hash32>,
    last_block_hash: Hash32,
    next_height: u64,
}

impl Sequencer {
    pub fn new(config: SequencerConfig) -> Self {
        Self {
            config,
            state: State::default(),
            mempool: Mempool::default(),
            executor: DeterministicExecutor,
            committed_deposits: BTreeSet::new(),
            last_block_hash: Hash32::ZERO,
            next_height: 0,
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

        let prev_state_root = self.state.root_hash();
        let txs = self.mempool.select_block(self.config.max_txs_per_block);
        let mut receipts = Vec::with_capacity(txs.len());
        let mut withdrawals = Vec::new();

        for tx in &txs {
            if let Err(reason) = self.verify_tx(tx) {
                receipts.push(Receipt::rejected(tx.tx_hash(), reason));
                continue;
            }

            let outcome = self.executor.apply(
                &mut self.state,
                tx,
                &ExecutionConfig {
                    block_time: timestamp,
                    block_height: self.next_height,
                    gas_coin_asset: self.config.gas_coin_asset,
                    max_internal_messages: self.config.max_internal_messages,
                },
            );
            receipts.push(outcome.receipt);
            withdrawals.extend(outcome.withdrawals);
        }

        let state_root = self.state.root_hash();
        let data_hash = canonical_data_hash(&txs, &receipts);
        let block = L2Block::new(
            self.next_height,
            self.last_block_hash,
            prev_state_root,
            state_root,
            txs,
            receipts,
            withdrawals,
            data_hash,
            timestamp,
        );

        self.last_block_hash = block.header.block_hash();
        self.next_height += 1;
        Some(block)
    }

    fn verify_tx(&self, tx: &SignedL2Transaction) -> Result<(), &'static str> {
        if tx.chain_id != self.config.chain_id {
            return Err("wrong_chain_id");
        }

        if tx.is_system() {
            return Ok(());
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

fn canonical_data_hash(txs: &[SignedL2Transaction], receipts: &[Receipt]) -> Hash32 {
    let tx_bytes = serde_json::to_vec(txs).expect("transactions are serializable");
    let receipt_bytes = serde_json::to_vec(receipts).expect("receipts are serializable");
    hash_domain("l2.batch.data", &[&tx_bytes, &receipt_bytes])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::{derive_account_id, sha256_bytes};
    use crate::merkle::verify_merkle_proof;
    use crate::types::{
        L2TransactionKind, ReceiptStatus, SignedL2Transaction, L2_NATIVE_GAS_ASSET,
    };
    use ed25519_dalek::{Signer, SigningKey};
    use rand_core::OsRng;

    fn signed_tx(
        signing_key: &SigningKey,
        from: Hash32,
        nonce: u64,
        kind: L2TransactionKind,
    ) -> SignedL2Transaction {
        let public_key = signing_key.verifying_key().to_bytes();
        let mut tx = SignedL2Transaction {
            chain_id: "ton-l2-devnet".to_owned(),
            from: Some(from),
            nonce,
            gas_limit: 1_000,
            max_gas_price: 1,
            kind,
            public_key: Some(hex::encode(public_key)),
            signature: None,
        };
        let signature = signing_key.sign(&tx.signing_payload());
        tx.signature = Some(hex::encode(signature.to_bytes()));
        tx
    }

    #[test]
    fn deposit_transfer_withdraw_block_flow() {
        let mut sequencer = Sequencer::new(SequencerConfig::default());
        let signing_key = SigningKey::generate(&mut OsRng);
        let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
        let recipient = sha256_bytes(b"recipient");

        sequencer.ingest_deposits(vec![DepositEvent {
            deposit_id: sha256_bytes(b"deposit-1"),
            asset_id: L2_NATIVE_GAS_ASSET,
            recipient: account_id,
            amount: 1_000,
            l1_tx_hash: sha256_bytes(b"l1-tx"),
            l1_lt: 7,
        }]);

        let block = sequencer.produce_block(100).expect("deposit block");
        assert_eq!(block.header.height, 0);
        assert_eq!(
            sequencer.state.account(account_id).unwrap().balance(0),
            1_000
        );

        sequencer.submit_tx(signed_tx(
            &signing_key,
            account_id,
            0,
            L2TransactionKind::Transfer {
                to: recipient,
                asset_id: L2_NATIVE_GAS_ASSET,
                amount: 100,
            },
        ));
        sequencer.submit_tx(signed_tx(
            &signing_key,
            account_id,
            1,
            L2TransactionKind::Withdraw {
                asset_id: L2_NATIVE_GAS_ASSET,
                amount: 50,
                l1_recipient: "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c".to_owned(),
            },
        ));

        let block = sequencer.produce_block(200).expect("user block");
        assert_eq!(block.receipts[0].status, ReceiptStatus::Applied);
        assert_eq!(block.receipts[1].status, ReceiptStatus::Applied);
        assert_eq!(block.withdrawals.len(), 1);

        let proof = block
            .withdrawal_proof(block.withdrawals[0].withdrawal_id)
            .expect("withdrawal proof");
        assert!(verify_merkle_proof(
            proof.withdrawal_root,
            proof.leaf.leaf_hash(),
            &proof.proof
        ));
    }

    #[test]
    fn duplicate_deposit_is_idempotent() {
        let mut sequencer = Sequencer::new(SequencerConfig::default());
        let recipient = sha256_bytes(b"recipient");
        let event = DepositEvent {
            deposit_id: sha256_bytes(b"deposit-1"),
            asset_id: 0,
            recipient,
            amount: 10,
            l1_tx_hash: sha256_bytes(b"l1-tx"),
            l1_lt: 1,
        };

        sequencer.ingest_deposits(vec![event.clone(), event]);
        sequencer.produce_block(1).expect("block");
        assert_eq!(sequencer.state.account(recipient).unwrap().balance(0), 10);
    }

    #[test]
    fn wrong_nonce_is_rejected() {
        let mut sequencer = Sequencer::new(SequencerConfig::default());
        let signing_key = SigningKey::generate(&mut OsRng);
        let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());

        sequencer.ingest_deposits(vec![DepositEvent {
            deposit_id: sha256_bytes(b"deposit-1"),
            asset_id: 0,
            recipient: account_id,
            amount: 1_000,
            l1_tx_hash: sha256_bytes(b"l1"),
            l1_lt: 1,
        }]);
        sequencer.produce_block(1);

        sequencer.submit_tx(signed_tx(
            &signing_key,
            account_id,
            9,
            L2TransactionKind::Transfer {
                to: sha256_bytes(b"other"),
                asset_id: 0,
                amount: 1,
            },
        ));
        let block = sequencer.produce_block(2).expect("block");
        assert_eq!(block.receipts[0].status, ReceiptStatus::Rejected);
        assert_eq!(block.receipts[0].reason.as_deref(), Some("bad_nonce"));
    }
}
