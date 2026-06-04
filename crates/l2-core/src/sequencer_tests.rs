use super::*;
use crate::crypto::{derive_account_id, sha256_bytes};
use crate::types::{L2TransactionKind, ReceiptStatus, SignedL2Transaction, L2_NATIVE_GAS_ASSET};
use crate::withdrawal::verify_withdrawal_merkle_proof;
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
    assert!(
        verify_withdrawal_merkle_proof(proof.withdrawal_root, &proof.leaf, &proof.proof)
            .expect("valid withdrawal proof encoding")
    );
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

#[test]
fn block_gas_limit_rejects_excess_transactions() {
    let mut sequencer = Sequencer::new(SequencerConfig {
        block_gas_limit: 10,
        ..SequencerConfig::default()
    });
    let signing_key = SigningKey::generate(&mut OsRng);
    let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
    let recipient = sha256_bytes(b"recipient");

    sequencer.ingest_deposits(vec![DepositEvent {
        deposit_id: sha256_bytes(b"deposit-1"),
        asset_id: 0,
        recipient: account_id,
        amount: 1_000,
        l1_tx_hash: sha256_bytes(b"l1"),
        l1_lt: 1,
    }]);
    sequencer.produce_block(1);

    for nonce in 0..2 {
        sequencer.submit_tx(signed_tx(
            &signing_key,
            account_id,
            nonce,
            L2TransactionKind::Transfer {
                to: recipient,
                asset_id: 0,
                amount: 1,
            },
        ));
    }

    let block = sequencer.produce_block(2).expect("block");
    assert_eq!(block.receipts[0].status, ReceiptStatus::Applied);
    assert_eq!(block.receipts[1].status, ReceiptStatus::Rejected);
    assert_eq!(
        block.receipts[1].reason.as_deref(),
        Some("block_gas_limit_exceeded")
    );
}

#[test]
fn public_deposit_transaction_is_rejected() {
    let mut sequencer = Sequencer::new(SequencerConfig::default());
    let recipient = sha256_bytes(b"recipient");
    let tx = SignedL2Transaction::system_deposit(
        "ton-l2-devnet",
        sha256_bytes(b"forged-public-deposit"),
        0,
        recipient,
        10_000,
    );

    sequencer.submit_tx(tx);
    let block = sequencer.produce_block(1).expect("block");

    assert_eq!(block.receipts[0].status, ReceiptStatus::Rejected);
    assert_eq!(
        block.receipts[0].reason.as_deref(),
        Some("deposit_must_be_system")
    );
    assert!(sequencer.state.account(recipient).is_none());
}

#[test]
fn overflowing_deposit_is_rejected_without_panic() {
    let mut sequencer = Sequencer::new(SequencerConfig::default());
    let recipient = sha256_bytes(b"recipient");

    sequencer.ingest_deposits(vec![DepositEvent {
        deposit_id: sha256_bytes(b"deposit-max"),
        asset_id: 0,
        recipient,
        amount: u128::MAX,
        l1_tx_hash: sha256_bytes(b"l1-a"),
        l1_lt: 1,
    }]);
    sequencer.produce_block(1).expect("first deposit block");

    sequencer.ingest_deposits(vec![DepositEvent {
        deposit_id: sha256_bytes(b"deposit-overflow"),
        asset_id: 0,
        recipient,
        amount: 1,
        l1_tx_hash: sha256_bytes(b"l1-b"),
        l1_lt: 2,
    }]);

    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sequencer.produce_block(2)));
    assert!(result.is_ok(), "overflowing deposit must not panic");

    let block = result.unwrap().expect("second deposit block");
    assert_eq!(block.receipts[0].status, ReceiptStatus::Rejected);
    assert_eq!(
        block.receipts[0].reason.as_deref(),
        Some("balance_overflow")
    );
    assert_eq!(
        sequencer.state.account(recipient).unwrap().balance(0),
        u128::MAX
    );
}
