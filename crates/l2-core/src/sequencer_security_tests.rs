use super::*;
use crate::crypto::{derive_account_id, sha256_bytes, Hash32};
use crate::state::{AccountFlags, AccountRecoveryLock, AccountType};
use crate::types::{
    L2TransactionKind, ReceiptStatus, SignedL2Transaction, L2_NATIVE_GAS_ASSET,
    L2_TRANSACTION_KIND_VERSION_V1, L2_TX_DOMAIN_SEPARATOR, L2_TX_VERSION_V2,
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

fn funded_public_account(sequencer: &mut Sequencer, signing_key: &SigningKey) -> Hash32 {
    let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
    assert!(sequencer
        .state
        .account_mut(account_id)
        .credit(L2_NATIVE_GAS_ASSET, 1_000));
    account_id
}

#[test]
fn mismatched_public_key_cannot_spoof_sender_account() {
    let mut sequencer = Sequencer::new(SequencerConfig::default());
    let owner_key = SigningKey::generate(&mut OsRng);
    let attacker_key = SigningKey::generate(&mut OsRng);
    let account_id = funded_public_account(&mut sequencer, &owner_key);
    let recipient = sha256_bytes(b"recipient");

    sequencer.submit_tx(signed_tx(
        &attacker_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: recipient,
            asset_id: L2_NATIVE_GAS_ASSET,
            amount: 1,
        },
    ));

    let block = sequencer.produce_block(1).expect("rejected block");
    assert_eq!(block.receipts[0].status, ReceiptStatus::Rejected);
    assert_eq!(
        block.receipts[0].reason.as_deref(),
        Some("public_key_sender_mismatch")
    );
    assert_eq!(sequencer.state.account(account_id).unwrap().nonce, 0);
    assert!(sequencer.state.account(recipient).is_none());
}

#[test]
fn recovery_locked_account_cannot_rotate_or_transfer() {
    let mut sequencer = Sequencer::new(SequencerConfig::default());
    let signing_key = SigningKey::generate(&mut OsRng);
    let new_key = SigningKey::generate(&mut OsRng);
    let account_id = funded_public_account(&mut sequencer, &signing_key);
    let recipient = sha256_bytes(b"recipient");
    sequencer.state.account_mut(account_id).recovery_lock = Some(AccountRecoveryLock {
        locked: true,
        admin: Some(sha256_bytes(b"recovery-admin")),
    });

    sequencer.submit_tx(signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::RotatePublicKey {
            new_public_key: hex::encode(new_key.verifying_key().to_bytes()),
        },
    ));
    sequencer.submit_tx(signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: recipient,
            asset_id: L2_NATIVE_GAS_ASSET,
            amount: 1,
        },
    ));

    let block = sequencer.produce_block(1).expect("rejected block");
    assert_eq!(block.receipts.len(), 2);
    for receipt in &block.receipts {
        assert_eq!(receipt.status, ReceiptStatus::Rejected);
        assert_eq!(receipt.reason.as_deref(), Some("account_recovery_locked"));
    }
    let account = sequencer.state.account(account_id).unwrap();
    assert_eq!(account.nonce, 0);
    assert_eq!(account.active_public_key, None);
    assert!(sequencer.state.account(recipient).is_none());
}

#[test]
fn system_account_cannot_submit_public_transactions() {
    let mut sequencer = Sequencer::new(SequencerConfig::default());
    let signing_key = SigningKey::generate(&mut OsRng);
    let account_id = funded_public_account(&mut sequencer, &signing_key);
    let account = sequencer.state.account_mut(account_id);
    account.account_type = AccountType::System;
    account.flags = AccountFlags {
        system_only: true,
        ..AccountFlags::default()
    };

    sequencer.submit_tx(signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: sha256_bytes(b"recipient"),
            asset_id: L2_NATIVE_GAS_ASSET,
            amount: 1,
        },
    ));

    let block = sequencer.produce_block(1).expect("rejected block");
    assert_eq!(block.receipts[0].status, ReceiptStatus::Rejected);
    assert_eq!(
        block.receipts[0].reason.as_deref(),
        Some("sender_system_only")
    );
    assert_eq!(sequencer.state.account(account_id).unwrap().nonce, 0);
}

#[test]
fn operator_account_can_submit_public_transactions() {
    let mut sequencer = Sequencer::new(SequencerConfig::default());
    let signing_key = SigningKey::generate(&mut OsRng);
    let account_id = funded_public_account(&mut sequencer, &signing_key);
    sequencer.state.account_mut(account_id).account_type = AccountType::Operator;
    let recipient = sha256_bytes(b"recipient");

    sequencer.submit_tx(signed_tx(
        &signing_key,
        account_id,
        0,
        L2TransactionKind::Transfer {
            to: recipient,
            asset_id: L2_NATIVE_GAS_ASSET,
            amount: 10,
        },
    ));

    let block = sequencer.produce_block(1).expect("operator block");
    assert_eq!(block.receipts[0].status, ReceiptStatus::Applied);
    assert_eq!(sequencer.state.account(account_id).unwrap().nonce, 1);
    assert_eq!(
        sequencer
            .state
            .account(recipient)
            .unwrap()
            .balance(L2_NATIVE_GAS_ASSET),
        10
    );
}
