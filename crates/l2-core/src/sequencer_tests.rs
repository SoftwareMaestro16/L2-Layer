use super::*;
use crate::crypto::{derive_account_id, sha256_bytes};
use crate::state::AccountType;
use crate::types::{
    L2TransactionKind, ReceiptStatus, SignedL2Transaction, L2_NATIVE_GAS_ASSET,
    L2_TRANSACTION_KIND_VERSION_V1, L2_TX_DOMAIN_SEPARATOR, L2_TX_VERSION_V2,
};
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
        tx_version: L2_TX_VERSION_V2,
        domain_separator: L2_TX_DOMAIN_SEPARATOR.to_owned(),
        chain_id: "ton-l2-devnet".to_owned(),
        from: Some(from),
        nonce,
        valid_until_block: u64::MAX,
        gas_limit: 1_000,
        max_gas_price: 1,
        fee_asset_id: L2_NATIVE_GAS_ASSET,
        memo_hash: None,
        transaction_kind_version: L2_TRANSACTION_KIND_VERSION_V1,
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
fn reserved_zero_address_deposit_is_rejected_by_sequencer() {
    let mut sequencer = Sequencer::new(SequencerConfig::default());

    sequencer.ingest_deposits(vec![DepositEvent {
        deposit_id: sha256_bytes(b"zero-deposit"),
        asset_id: 0,
        recipient: Hash32::ZERO,
        amount: 10,
        l1_tx_hash: sha256_bytes(b"l1-tx"),
        l1_lt: 1,
    }]);

    let block = sequencer.produce_block(1).expect("block");
    assert_eq!(block.receipts[0].status, ReceiptStatus::Rejected);
    assert_eq!(
        block.receipts[0].reason.as_deref(),
        Some("reserved_zero_address")
    );
    assert!(sequencer.state.account(Hash32::ZERO).is_none());
}

#[test]
fn reserved_zero_address_transfer_is_rejected_by_sequencer_before_execution() {
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
    sequencer.produce_block(1).expect("deposit block");

    sequencer.submit_tx(signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: Hash32::ZERO,
            asset_id: 0,
            amount: 1,
        },
    ));

    let block = sequencer.produce_block(2).expect("user block");
    assert_eq!(block.receipts[0].status, ReceiptStatus::Rejected);
    assert_eq!(
        block.receipts[0].reason.as_deref(),
        Some("reserved_zero_address")
    );
    assert_eq!(sequencer.state.account(account_id).unwrap().nonce, 0);
    assert!(sequencer.state.account(Hash32::ZERO).is_none());
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
fn expired_transaction_is_rejected_with_canonical_reason() {
    let mut sequencer = Sequencer::new(SequencerConfig::default());
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
    sequencer.produce_block(1).expect("deposit block");

    let mut tx = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: recipient,
            asset_id: 0,
            amount: 1,
        },
    );
    tx.valid_until_block = 0;
    tx.signature = None;
    let signature = signing_key.sign(&tx.signing_payload());
    tx.signature = Some(hex::encode(signature.to_bytes()));
    sequencer.submit_tx(tx);

    let block = sequencer.produce_block(2).expect("block");
    assert_eq!(block.receipts[0].status, ReceiptStatus::Rejected);
    assert_eq!(block.receipts[0].reason.as_deref(), Some("tx_expired"));
    assert_eq!(sequencer.state.account(account_id).unwrap().nonce, 0);
    assert!(sequencer.state.account(recipient).is_none());
}

#[test]
fn duplicate_transaction_hash_in_same_batch_is_rejected_before_nonce_check() {
    let mut sequencer = Sequencer::new(SequencerConfig::default());
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
    sequencer.produce_block(1).expect("deposit block");

    let tx = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: recipient,
            asset_id: 0,
            amount: 1,
        },
    );
    sequencer.submit_tx(tx.clone());
    sequencer.submit_tx(tx);

    let block = sequencer.produce_block(2).expect("block");
    assert_eq!(block.receipts[0].status, ReceiptStatus::Applied);
    assert_eq!(block.receipts[1].status, ReceiptStatus::Rejected);
    assert_eq!(block.receipts[1].reason.as_deref(), Some("duplicate_tx"));
    assert_eq!(sequencer.state.account(account_id).unwrap().nonce, 1);
}

#[test]
fn unsupported_fee_asset_is_rejected_without_nonce_increment() {
    let mut sequencer = Sequencer::new(SequencerConfig::default());
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
    sequencer.produce_block(1).expect("deposit block");

    let mut tx = signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: recipient,
            asset_id: 0,
            amount: 1,
        },
    );
    tx.fee_asset_id = 1;
    tx.signature = None;
    let signature = signing_key.sign(&tx.signing_payload());
    tx.signature = Some(hex::encode(signature.to_bytes()));
    sequencer.submit_tx(tx);

    let block = sequencer.produce_block(2).expect("block");
    assert_eq!(block.receipts[0].status, ReceiptStatus::Rejected);
    assert_eq!(
        block.receipts[0].reason.as_deref(),
        Some("unsupported_fee_asset")
    );
    assert_eq!(sequencer.state.account(account_id).unwrap().nonce, 0);
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

#[test]
fn public_key_rotation_changes_authorized_signer_without_changing_account_id() {
    let mut sequencer = Sequencer::new(SequencerConfig::default());
    let old_key = SigningKey::generate(&mut OsRng);
    let new_key = SigningKey::generate(&mut OsRng);
    let account_id = derive_account_id(&old_key.verifying_key().to_bytes());
    let recipient = sha256_bytes(b"recipient");

    sequencer.ingest_deposits(vec![DepositEvent {
        deposit_id: sha256_bytes(b"deposit-1"),
        asset_id: 0,
        recipient: account_id,
        amount: 1_000,
        l1_tx_hash: sha256_bytes(b"l1"),
        l1_lt: 1,
    }]);
    sequencer.produce_block(1).expect("deposit block");

    sequencer.submit_tx(signed_tx(
        &old_key,
        account_id,
        0,
        L2TransactionKind::RotatePublicKey {
            new_public_key: hex::encode(new_key.verifying_key().to_bytes()),
        },
    ));
    let rotation_block = sequencer.produce_block(2).expect("rotation block");
    assert_eq!(rotation_block.receipts[0].status, ReceiptStatus::Applied);
    assert_eq!(
        sequencer
            .state
            .account(account_id)
            .unwrap()
            .active_public_key,
        Some(Hash32::new(new_key.verifying_key().to_bytes()))
    );

    sequencer.submit_tx(signed_tx(
        &new_key,
        account_id,
        1,
        L2TransactionKind::Transfer {
            to: recipient,
            asset_id: 0,
            amount: 10,
        },
    ));
    let new_key_block = sequencer.produce_block(3).expect("new key block");
    assert_eq!(new_key_block.receipts[0].status, ReceiptStatus::Applied);

    sequencer.submit_tx(signed_tx(
        &old_key,
        account_id,
        2,
        L2TransactionKind::Transfer {
            to: recipient,
            asset_id: 0,
            amount: 10,
        },
    ));
    let stale_key_block = sequencer.produce_block(4).expect("stale key block");
    assert_eq!(stale_key_block.receipts[0].status, ReceiptStatus::Rejected);
    assert_eq!(
        stale_key_block.receipts[0].reason.as_deref(),
        Some("public_key_sender_mismatch")
    );
}

#[test]
fn contract_account_cannot_masquerade_as_user_sender() {
    let mut sequencer = Sequencer::new(SequencerConfig::default());
    let signing_key = SigningKey::generate(&mut OsRng);
    let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
    let recipient = sha256_bytes(b"recipient");
    {
        let account = sequencer.state.account_mut(account_id);
        account.account_type = AccountType::Contract;
        account.flags.contract_only = true;
        account.credit(0, 1_000);
    }

    sequencer.submit_tx(signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: recipient,
            asset_id: 0,
            amount: 1,
        },
    ));

    let block = sequencer.produce_block(1).expect("block");
    assert_eq!(block.receipts[0].status, ReceiptStatus::Rejected);
    assert_eq!(
        block.receipts[0].reason.as_deref(),
        Some("sender_contract_only")
    );
    assert_eq!(sequencer.state.account(account_id).unwrap().nonce, 0);
}
