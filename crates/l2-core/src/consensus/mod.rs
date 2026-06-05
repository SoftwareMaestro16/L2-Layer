use crate::crypto::{hash_domain, Hash32};
use crate::state::Account;
use crate::types::{
    L2BlockHeader, L2Event, L2TransactionKind, Receipt, ReceiptStatus, SignedL2Transaction,
    UnsignedL2Transaction, WithdrawalLeaf,
};
use std::collections::BTreeMap;

pub const CONSENSUS_ENCODING_VERSION: u8 = 1;

const MAGIC: &[u8; 4] = b"EL2C";
const TYPE_UNSIGNED_TX: u8 = 0x01;
const TYPE_RECEIPT: u8 = 0x02;
const TYPE_WITHDRAWAL_LEAF: u8 = 0x03;
const TYPE_ACCOUNT_LEAF: u8 = 0x04;
const TYPE_BLOCK_HEADER: u8 = 0x05;
const TYPE_BATCH_DATA: u8 = 0x06;
const TYPE_SIGNED_TX: u8 = 0x07;

const KIND_DEPOSIT: u8 = 0x01;
const KIND_TRANSFER: u8 = 0x02;
const KIND_WITHDRAW: u8 = 0x03;
const KIND_CALL_CONTRACT: u8 = 0x04;
const KIND_DEPLOY_CONTRACT: u8 = 0x05;
const KIND_ROTATE_PUBLIC_KEY: u8 = 0x06;
const KIND_INTERNAL_MESSAGE: u8 = 0x07;

const STATUS_APPLIED: u8 = 0x01;
const STATUS_REJECTED: u8 = 0x02;

const EVENT_CONTRACT_DEPLOYED: u8 = 0x01;
const EVENT_CONTRACT_CALLED: u8 = 0x02;
const EVENT_WITHDRAWAL_CREATED: u8 = 0x03;

pub fn encode_unsigned_transaction(tx: &UnsignedL2Transaction) -> Vec<u8> {
    let mut out = with_header(TYPE_UNSIGNED_TX);
    write_unsigned_transaction_body(&mut out, tx);
    out
}

pub fn encode_signed_transaction(tx: &SignedL2Transaction) -> Vec<u8> {
    let mut out = with_header(TYPE_SIGNED_TX);
    write_unsigned_transaction_body(&mut out, &tx.unsigned());
    write_optional_string(&mut out, tx.public_key.as_deref());
    write_optional_string(&mut out, tx.signature.as_deref());
    out
}

pub fn transaction_hash(tx: &SignedL2Transaction) -> Hash32 {
    hash_domain("l2.tx.v2", &[&encode_unsigned_transaction(&tx.unsigned())])
}

pub fn signing_payload(tx: &SignedL2Transaction) -> Vec<u8> {
    encode_unsigned_transaction(&tx.unsigned())
}

pub fn encode_receipt(receipt: &Receipt) -> Vec<u8> {
    let mut out = with_header(TYPE_RECEIPT);
    write_hash(&mut out, receipt.tx_hash);
    out.push(match receipt.status {
        ReceiptStatus::Applied => STATUS_APPLIED,
        ReceiptStatus::Rejected => STATUS_REJECTED,
    });
    write_u128(&mut out, receipt.gas_charged);
    write_optional_string(&mut out, receipt.reason.as_deref());
    write_optional_hash(&mut out, receipt.withdrawal_id);
    write_len(&mut out, receipt.events.len());
    for event in &receipt.events {
        encode_l2_event(&mut out, event);
    }
    out
}

pub fn receipt_leaf_hash(receipt: &Receipt) -> Hash32 {
    hash_domain("l2.receipt.leaf.v1", &[&encode_receipt(receipt)])
}

pub fn encode_withdrawal_leaf(leaf: &WithdrawalLeaf) -> Vec<u8> {
    let mut out = with_header(TYPE_WITHDRAWAL_LEAF);
    write_hash(&mut out, leaf.withdrawal_id);
    write_u32(&mut out, leaf.asset_id);
    write_u128(&mut out, leaf.amount);
    write_hash(&mut out, leaf.l2_sender);
    write_string(&mut out, &leaf.l1_recipient);
    out
}

pub fn withdrawal_leaf_hash(leaf: &WithdrawalLeaf) -> Hash32 {
    hash_domain("l2.withdrawal.leaf.v1", &[&encode_withdrawal_leaf(leaf)])
}

pub fn withdrawal_id(
    tx_hash: Hash32,
    asset_id: u32,
    amount: u128,
    l2_sender: Hash32,
    l1_recipient: &str,
) -> Hash32 {
    let mut bytes = Vec::new();
    write_hash(&mut bytes, tx_hash);
    write_u32(&mut bytes, asset_id);
    write_u128(&mut bytes, amount);
    write_hash(&mut bytes, l2_sender);
    write_string(&mut bytes, l1_recipient);
    hash_domain("l2.withdrawal.id.v1", &[&bytes])
}

pub fn encode_account_leaf(account_id: Hash32, account: &Account) -> Vec<u8> {
    let mut out = with_header(TYPE_ACCOUNT_LEAF);
    write_hash(&mut out, account_id);
    out.push(account.account_type.consensus_tag());
    out.push(account.flags.consensus_bits());
    write_optional_hash(&mut out, account.active_public_key);
    write_optional_recovery_lock(&mut out, account);
    write_u64(&mut out, account.nonce);
    write_balances(&mut out, &account.balances);
    write_hash(&mut out, account.code_hash);
    write_hash(&mut out, account.data_hash);
    write_hash(&mut out, account.storage_root);
    write_u64(&mut out, account.last_lt);
    out
}

pub fn account_leaf_hash(account_id: Hash32, account: &Account) -> Hash32 {
    hash_domain(
        "l2.state.account.v1",
        &[&encode_account_leaf(account_id, account)],
    )
}

pub fn encode_block_header(header: &L2BlockHeader) -> Vec<u8> {
    let mut out = with_header(TYPE_BLOCK_HEADER);
    write_u64(&mut out, header.height);
    write_hash(&mut out, header.prev_block_hash);
    write_hash(&mut out, header.prev_state_root);
    write_hash(&mut out, header.state_root);
    write_hash(&mut out, header.tx_root);
    write_hash(&mut out, header.receipt_root);
    write_hash(&mut out, header.withdrawal_root);
    write_hash(&mut out, header.data_hash);
    write_u64(&mut out, header.timestamp);
    out
}

pub fn block_header_hash(header: &L2BlockHeader) -> Hash32 {
    hash_domain("l2.block.header.v1", &[&encode_block_header(header)])
}

pub fn encode_batch_data(txs: &[SignedL2Transaction], receipts: &[Receipt]) -> Vec<u8> {
    let mut out = with_header(TYPE_BATCH_DATA);
    write_len(&mut out, txs.len());
    for tx in txs {
        write_bytes(&mut out, &encode_signed_transaction(tx));
    }
    write_len(&mut out, receipts.len());
    for receipt in receipts {
        write_bytes(&mut out, &encode_receipt(receipt));
    }
    out
}

pub fn batch_data_hash(txs: &[SignedL2Transaction], receipts: &[Receipt]) -> Hash32 {
    hash_domain("l2.batch.data.v1", &[&encode_batch_data(txs, receipts)])
}

pub fn derive_account_id(public_key: &[u8; 32]) -> Hash32 {
    hash_domain("l2.account.ed25519.v1", &[public_key])
}

fn write_unsigned_transaction_body(out: &mut Vec<u8>, tx: &UnsignedL2Transaction) {
    write_u16(out, tx.tx_version);
    write_string(out, &tx.domain_separator);
    write_string(out, &tx.chain_id);
    write_optional_hash(out, tx.from);
    write_u64(out, tx.nonce);
    write_u64(out, tx.valid_until_block);
    write_u64(out, tx.gas_limit);
    write_u128(out, tx.max_gas_price);
    write_u32(out, tx.fee_asset_id);
    write_optional_hash(out, tx.memo_hash);
    write_u16(out, tx.transaction_kind_version);
    encode_transaction_kind(out, &tx.kind);
}

fn encode_transaction_kind(out: &mut Vec<u8>, kind: &L2TransactionKind) {
    match kind {
        L2TransactionKind::Deposit {
            deposit_id,
            asset_id,
            recipient,
            amount,
        } => {
            out.push(KIND_DEPOSIT);
            write_hash(out, *deposit_id);
            write_u32(out, *asset_id);
            write_hash(out, *recipient);
            write_u128(out, *amount);
        }
        L2TransactionKind::Transfer {
            to,
            asset_id,
            amount,
        } => {
            out.push(KIND_TRANSFER);
            write_hash(out, *to);
            write_u32(out, *asset_id);
            write_u128(out, *amount);
        }
        L2TransactionKind::RotatePublicKey { new_public_key } => {
            out.push(KIND_ROTATE_PUBLIC_KEY);
            write_string(out, new_public_key);
        }
        L2TransactionKind::Withdraw {
            asset_id,
            amount,
            l1_recipient,
        } => {
            out.push(KIND_WITHDRAW);
            write_u32(out, *asset_id);
            write_u128(out, *amount);
            write_string(out, l1_recipient);
        }
        L2TransactionKind::CallContract {
            contract,
            body_boc_base64,
        } => {
            out.push(KIND_CALL_CONTRACT);
            write_hash(out, *contract);
            write_string(out, body_boc_base64);
        }
        L2TransactionKind::DeployContract {
            contract,
            code_boc_base64,
            data_boc_base64,
        } => {
            out.push(KIND_DEPLOY_CONTRACT);
            write_hash(out, *contract);
            write_string(out, code_boc_base64);
            write_string(out, data_boc_base64);
        }
        L2TransactionKind::InternalMessage {
            message_id,
            from,
            to,
            value,
            body_boc_base64,
            bounce,
            bounced,
        } => {
            out.push(KIND_INTERNAL_MESSAGE);
            write_hash(out, *message_id);
            write_hash(out, *from);
            write_hash(out, *to);
            write_u128(out, *value);
            write_string(out, body_boc_base64);
            out.push(u8::from(*bounce));
            out.push(u8::from(*bounced));
        }
    }
}

fn encode_l2_event(out: &mut Vec<u8>, event: &L2Event) {
    match event {
        L2Event::ContractDeployed {
            contract,
            deployer,
            code_hash,
            data_hash,
        } => {
            out.push(EVENT_CONTRACT_DEPLOYED);
            write_hash(out, *contract);
            write_hash(out, *deployer);
            write_hash(out, *code_hash);
            write_hash(out, *data_hash);
        }
        L2Event::ContractCalled {
            contract,
            caller,
            body_hash,
        } => {
            out.push(EVENT_CONTRACT_CALLED);
            write_hash(out, *contract);
            write_hash(out, *caller);
            write_hash(out, *body_hash);
        }
        L2Event::WithdrawalCreated {
            withdrawal_id,
            asset_id,
            amount,
            l2_sender,
            l1_recipient,
        } => {
            out.push(EVENT_WITHDRAWAL_CREATED);
            write_hash(out, *withdrawal_id);
            write_u32(out, *asset_id);
            write_u128(out, *amount);
            write_hash(out, *l2_sender);
            write_string(out, l1_recipient);
        }
    }
}

fn write_balances(out: &mut Vec<u8>, balances: &BTreeMap<u32, u128>) {
    write_len(out, balances.len());
    for (asset_id, balance) in balances {
        write_u32(out, *asset_id);
        write_u128(out, *balance);
    }
}

fn with_header(type_tag: u8) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.push(CONSENSUS_ENCODING_VERSION);
    out.push(type_tag);
    out
}

fn write_hash(out: &mut Vec<u8>, hash: Hash32) {
    out.extend_from_slice(hash.as_bytes());
}

fn write_optional_hash(out: &mut Vec<u8>, hash: Option<Hash32>) {
    match hash {
        Some(hash) => {
            out.push(1);
            write_hash(out, hash);
        }
        None => out.push(0),
    }
}

fn write_optional_recovery_lock(out: &mut Vec<u8>, account: &Account) {
    match &account.recovery_lock {
        Some(lock) => {
            out.push(1);
            out.push(u8::from(lock.locked));
            write_optional_hash(out, lock.admin);
        }
        None => out.push(0),
    }
}

fn write_optional_string(out: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            out.push(1);
            write_string(out, value);
        }
        None => out.push(0),
    }
}

fn write_string(out: &mut Vec<u8>, value: &str) {
    write_bytes(out, value.as_bytes());
}

fn write_bytes(out: &mut Vec<u8>, value: &[u8]) {
    write_len(out, value.len());
    out.extend_from_slice(value);
}

fn write_len(out: &mut Vec<u8>, len: usize) {
    let len = u32::try_from(len).expect("consensus payload length exceeds u32");
    write_u32(out, len);
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn write_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn write_u128(out: &mut Vec<u8>, value: u128) {
    out.extend_from_slice(&value.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::sha256_bytes;
    use crate::types::{L2TransactionKind, L2_NATIVE_GAS_ASSET};

    #[test]
    fn unsigned_transaction_encoding_is_stable() {
        let tx = vector_transaction();

        assert_eq!(
            hex::encode(encode_unsigned_transaction(&tx.unsigned())),
            "454c32430101000200000011656e74726f7069732e6c322e74782e763200000010656e74726f7069732d746573746e657401aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0000000000000007000000000000006300000000000001f40000000000000000000000000000002a0000000001dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd000102bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb00000000000000000000000000000000000003e8"
        );
    }

    #[test]
    fn golden_vector_hashes_are_stable() {
        let tx = vector_transaction();
        let receipt = Receipt::applied(tx.tx_hash(), 10, Some(hash(0xcc)));
        let withdrawal = WithdrawalLeaf::new(
            tx.tx_hash(),
            L2_NATIVE_GAS_ASSET,
            55,
            hash(0xaa),
            "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c".to_owned(),
        );
        let header = L2BlockHeader {
            height: 9,
            prev_block_hash: hash(0x01),
            prev_state_root: hash(0x02),
            state_root: hash(0x03),
            tx_root: hash(0x04),
            receipt_root: hash(0x05),
            withdrawal_root: hash(0x06),
            data_hash: batch_data_hash(&[tx.clone()], &[receipt.clone()]),
            timestamp: 777,
        };
        let mut account = Account::default();
        account.nonce = 3;
        account.credit(L2_NATIVE_GAS_ASSET, 1_000);
        account.credit(2, 500);
        account.code_hash = hash(0x11);
        account.data_hash = hash(0x12);
        account.storage_root = hash(0x13);
        account.last_lt = 9;

        assert_eq!(
            tx.tx_hash().to_hex(),
            "039718f82070163d8d92c41fea4e14baf42be6b5ceb0d10a0d66d095eee77590"
        );
        assert_eq!(
            receipt.leaf_hash().to_hex(),
            "002b7a3abb022a944ab4060db65f6df449f2875528dbf4cd7047e7ee64281bb0"
        );
        assert_eq!(
            withdrawal.leaf_hash().to_hex(),
            "978ae37e92dca1a024d86eb82b37cf474766d26c448460de0f247ffe140cd5d0"
        );
        assert_eq!(
            header.block_hash().to_hex(),
            "dbc739765aaba3517b6ffe7cea1e5f7f6ab5711e677cc95a3f440be6ac2b425d"
        );
        assert_eq!(
            account_leaf_hash(hash(0xaa), &account).to_hex(),
            "2b283b553c4d56e5ee8054b55601397d27c9ce00b4620a7001f2f44c538e9331"
        );
    }

    #[test]
    fn optional_fields_have_distinct_canonical_bytes() {
        let mut without_sender = vector_transaction();
        without_sender.from = None;
        let with_sender = vector_transaction();

        assert_ne!(
            encode_unsigned_transaction(&without_sender.unsigned()),
            encode_unsigned_transaction(&with_sender.unsigned())
        );
    }

    #[test]
    fn envelope_fields_are_bound_to_signing_payload() {
        let base = vector_transaction();
        let mut changed_version = base.clone();
        changed_version.tx_version = 3;
        let mut changed_expiration = base.clone();
        changed_expiration.valid_until_block += 1;
        let mut changed_fee_asset = base.clone();
        changed_fee_asset.fee_asset_id = 1;
        let mut without_memo = base.clone();
        without_memo.memo_hash = None;

        assert_ne!(
            encode_unsigned_transaction(&base.unsigned()),
            encode_unsigned_transaction(&changed_version.unsigned())
        );
        assert_ne!(base.tx_hash(), changed_version.tx_hash());
        assert_ne!(base.tx_hash(), changed_expiration.tx_hash());
        assert_ne!(base.tx_hash(), changed_fee_asset.tx_hash());
        assert_ne!(base.tx_hash(), without_memo.tx_hash());
    }

    #[test]
    fn signed_auth_fields_do_not_change_transaction_hash() {
        let mut first = vector_transaction();
        let mut second = first.clone();
        first.public_key = Some("aa".repeat(32));
        first.signature = Some("bb".repeat(64));
        second.public_key = Some("cc".repeat(32));
        second.signature = Some("dd".repeat(64));

        assert_eq!(first.tx_hash(), second.tx_hash());
        assert_ne!(
            encode_signed_transaction(&first),
            encode_signed_transaction(&second)
        );
    }

    #[test]
    fn receipt_events_are_part_of_leaf_hash() {
        let tx_hash = hash(0xaa);
        let plain = Receipt::applied(tx_hash, 10, None);
        let with_event =
            Receipt::applied(tx_hash, 10, None).with_events(vec![L2Event::ContractCalled {
                contract: hash(0x01),
                caller: hash(0x02),
                body_hash: hash(0x03),
            }]);

        assert_ne!(plain.leaf_hash(), with_event.leaf_hash());
        assert!(encode_receipt(&with_event).len() > encode_receipt(&plain).len());
    }

    fn vector_transaction() -> SignedL2Transaction {
        SignedL2Transaction {
            tx_version: crate::types::L2_TX_VERSION_V2,
            domain_separator: crate::types::L2_TX_DOMAIN_SEPARATOR.to_owned(),
            chain_id: "entropis-testnet".to_owned(),
            from: Some(hash(0xaa)),
            nonce: 7,
            valid_until_block: 99,
            gas_limit: 500,
            max_gas_price: 42,
            fee_asset_id: L2_NATIVE_GAS_ASSET,
            memo_hash: Some(hash(0xdd)),
            transaction_kind_version: crate::types::L2_TRANSACTION_KIND_VERSION_V1,
            kind: L2TransactionKind::Transfer {
                to: hash(0xbb),
                asset_id: L2_NATIVE_GAS_ASSET,
                amount: 1_000,
            },
            public_key: None,
            signature: None,
        }
    }

    fn hash(byte: u8) -> Hash32 {
        Hash32::new([byte; 32])
    }

    #[test]
    fn account_id_derivation_is_versioned() {
        assert_eq!(
            derive_account_id(&[7; 32]),
            hash_domain("l2.account.ed25519.v1", &[&[7; 32]])
        );
        assert_ne!(derive_account_id(&[7; 32]), sha256_bytes(&[7; 32]));
    }
}
