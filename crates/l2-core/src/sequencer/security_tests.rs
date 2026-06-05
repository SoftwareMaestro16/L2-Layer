use super::*;
use crate::crypto::{derive_account_id, sha256_bytes};
use crate::types::{
    L2TransactionKind, ReceiptStatus, SignedL2Transaction, L2_NATIVE_GAS_ASSET,
    L2_TRANSACTION_KIND_VERSION_V1, L2_TX_DOMAIN_SEPARATOR, L2_TX_VERSION_V2,
};
use ed25519_dalek::{Signer, SigningKey};
use rand_core::OsRng;

fn signed_transfer(
    signing_key: &SigningKey,
    from: Hash32,
    nonce: u64,
    to: Hash32,
    amount: u128,
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
        kind: L2TransactionKind::Transfer {
            to,
            asset_id: L2_NATIVE_GAS_ASSET,
            amount,
        },
        public_key: Some(hex::encode(public_key)),
        signature: None,
    };
    resign(&mut tx, signing_key);
    tx
}

fn resign(tx: &mut SignedL2Transaction, signing_key: &SigningKey) {
    tx.signature = None;
    tx.signature = Some(hex::encode(
        signing_key.sign(&tx.signing_payload()).to_bytes(),
    ));
}

fn funded_sequencer() -> (Sequencer, SigningKey, Hash32) {
    let mut sequencer = Sequencer::new(SequencerConfig::default());
    let signing_key = SigningKey::generate(&mut OsRng);
    let account_id = derive_account_id(&signing_key.verifying_key().to_bytes());
    sequencer.ingest_deposits(vec![DepositEvent {
        deposit_id: sha256_bytes(b"security-deposit"),
        asset_id: L2_NATIVE_GAS_ASSET,
        recipient: account_id,
        amount: 10_000,
        l1_tx_hash: sha256_bytes(b"security-l1"),
        l1_lt: 1,
    }]);
    sequencer.produce_block(1).expect("deposit block");
    (sequencer, signing_key, account_id)
}

#[test]
fn mutated_signed_payload_is_rejected_as_bad_signature() {
    let (mut sequencer, signing_key, account_id) = funded_sequencer();
    let recipient = sha256_bytes(b"recipient");
    let mut tx = signed_transfer(&signing_key, account_id, 0, recipient, 10);

    tx.kind = L2TransactionKind::Transfer {
        to: recipient,
        asset_id: L2_NATIVE_GAS_ASSET,
        amount: 11,
    };
    sequencer.submit_tx(tx);

    let block = sequencer.produce_block(2).expect("block");
    assert_eq!(block.receipts[0].status, ReceiptStatus::Rejected);
    assert_eq!(block.receipts[0].reason.as_deref(), Some("bad_signature"));
    assert_eq!(sequencer.state.account(account_id).unwrap().nonce, 0);
    assert!(sequencer.state.account(recipient).is_none());
}

#[test]
fn envelope_version_domain_and_kind_version_are_rejected_before_nonce_advance() {
    let (mut sequencer, signing_key, account_id) = funded_sequencer();
    let recipient = sha256_bytes(b"recipient");
    let mut wrong_version = signed_transfer(&signing_key, account_id, 0, recipient, 1);
    wrong_version.tx_version = L2_TX_VERSION_V2 + 1;
    resign(&mut wrong_version, &signing_key);
    let mut wrong_domain = signed_transfer(&signing_key, account_id, 0, recipient, 1);
    wrong_domain.domain_separator = "entropis.l2.tx.v3".to_owned();
    resign(&mut wrong_domain, &signing_key);
    let mut wrong_kind_version = signed_transfer(&signing_key, account_id, 0, recipient, 1);
    wrong_kind_version.transaction_kind_version = L2_TRANSACTION_KIND_VERSION_V1 + 1;
    resign(&mut wrong_kind_version, &signing_key);

    sequencer.submit_tx(wrong_version);
    sequencer.submit_tx(wrong_domain);
    sequencer.submit_tx(wrong_kind_version);

    let block = sequencer.produce_block(2).expect("block");
    let reasons = block
        .receipts
        .iter()
        .map(|receipt| receipt.reason.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(
        reasons,
        vec![
            Some("unsupported_tx_version"),
            Some("invalid_domain_separator"),
            Some("unsupported_transaction_kind_version"),
        ]
    );
    assert_eq!(sequencer.state.account(account_id).unwrap().nonce, 0);
    assert!(sequencer.state.account(recipient).is_none());
}

#[test]
fn valid_until_block_is_inclusive_at_current_block_height() {
    let (mut sequencer, signing_key, account_id) = funded_sequencer();
    let recipient = sha256_bytes(b"recipient");
    let mut tx = signed_transfer(&signing_key, account_id, 0, recipient, 10);
    tx.valid_until_block = 1;
    resign(&mut tx, &signing_key);

    sequencer.submit_tx(tx);

    let block = sequencer.produce_block(2).expect("block");
    assert_eq!(block.header.height, 1);
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
