use crate::crypto::Hash32;
use crate::state::Account;
use crate::tvm::{decode_contract_cell_boc_base64, DEFAULT_MAX_TVM_BOC_BYTES};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use num_bigint::BigUint;
use tonlib_core::cell::dict::predefined_readers::{key_reader_256bit, val_reader_uint};
use tonlib_core::cell::BagOfCells;

#[path = "enwallet/execute.rs"]
mod execute;

pub use execute::execute_enwallet_v5r1;

pub const ENWALLET_V5R1_INTERFACE: &str = "org.ton.wallet.v5.r1";
pub const ENWALLET_V5R1_LABEL: &str = "Wallet Signed External V5 R1";
pub const ENWALLET_V5R1_TESTNET_WALLET_ID: u32 = 0x7fff_fffd;
pub const ENWALLET_V5R1_CODE_HASH: Hash32 = Hash32([
    0x9a, 0xfa, 0xef, 0xf1, 0x0b, 0xb8, 0x34, 0xd0, 0xcf, 0xc3, 0x2f, 0x7b, 0x23, 0x0c, 0xde, 0xf5,
    0x30, 0xe6, 0x50, 0x44, 0x35, 0x2f, 0xc1, 0xf1, 0x96, 0xfb, 0x0c, 0xcb, 0x63, 0x24, 0xc5, 0xc8,
]);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnWalletV5State {
    pub is_signature_allowed: bool,
    pub seqno: u32,
    pub wallet_id: u32,
    pub public_key: Hash32,
    pub extensions_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum EnWalletReadError {
    #[error("unsupported EnWallet code hash")]
    UnsupportedCodeHash,
    #[error("EnWallet account has no data BoC")]
    MissingDataBoc,
    #[error("malformed EnWallet data BoC")]
    MalformedDataBoc,
    #[error("EnWallet data hash does not match account state")]
    DataHashMismatch,
}

pub fn is_enwallet_v5r1_code_hash(code_hash: Hash32) -> bool {
    code_hash == ENWALLET_V5R1_CODE_HASH
}

pub fn interface_for_code_hash(code_hash: Hash32) -> Option<(&'static str, &'static str)> {
    if is_enwallet_v5r1_code_hash(code_hash) {
        Some((ENWALLET_V5R1_INTERFACE, ENWALLET_V5R1_LABEL))
    } else {
        None
    }
}

pub fn read_enwallet_v5_state(account: &Account) -> Result<EnWalletV5State, EnWalletReadError> {
    if !is_enwallet_v5r1_code_hash(account.code_hash) {
        return Err(EnWalletReadError::UnsupportedCodeHash);
    }
    if account.data_hash != account.storage_root {
        return Err(EnWalletReadError::DataHashMismatch);
    }
    let data_boc_base64 = account
        .data_boc_base64
        .as_deref()
        .ok_or(EnWalletReadError::MissingDataBoc)?;
    let data_cell = decode_contract_cell_boc_base64(data_boc_base64, DEFAULT_MAX_TVM_BOC_BYTES)
        .map_err(|_| EnWalletReadError::MalformedDataBoc)?;
    if data_cell.cell_hash != account.data_hash {
        return Err(EnWalletReadError::DataHashMismatch);
    }
    decode_enwallet_v5_data_boc(&data_cell.boc_base64)
}

pub fn decode_enwallet_v5_data_boc(
    data_boc_base64: &str,
) -> Result<EnWalletV5State, EnWalletReadError> {
    let boc = BASE64_STANDARD
        .decode(data_boc_base64.as_bytes())
        .map_err(|_| EnWalletReadError::MalformedDataBoc)?;
    let root = BagOfCells::parse(&boc)
        .and_then(BagOfCells::single_root)
        .map_err(|_| EnWalletReadError::MalformedDataBoc)?;
    let mut parser = root.parser();
    let is_signature_allowed = parser
        .load_bit()
        .map_err(|_| EnWalletReadError::MalformedDataBoc)?;
    let seqno = parser
        .load_u32(32)
        .map_err(|_| EnWalletReadError::MalformedDataBoc)?;
    let wallet_id = parser
        .load_u32(32)
        .map_err(|_| EnWalletReadError::MalformedDataBoc)?;
    let public_key_bytes = parser
        .load_bits(256)
        .map_err(|_| EnWalletReadError::MalformedDataBoc)?;
    let public_key = public_key_from_bits(&public_key_bytes)?;
    let extensions = parser
        .load_dict(256, key_reader_256bit, val_reader_uint)
        .map_err(|_| EnWalletReadError::MalformedDataBoc)?;
    parser
        .ensure_empty()
        .map_err(|_| EnWalletReadError::MalformedDataBoc)?;

    Ok(EnWalletV5State {
        is_signature_allowed,
        seqno,
        wallet_id,
        public_key,
        extensions_count: extensions
            .values()
            .filter(|value| *value != &BigUint::from(0u8))
            .count(),
    })
}

fn public_key_from_bits(value: &[u8]) -> Result<Hash32, EnWalletReadError> {
    let bytes: [u8; 32] = value
        .try_into()
        .map_err(|_| EnWalletReadError::MalformedDataBoc)?;
    Ok(Hash32::new(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Account;
    use tonlib_core::cell::{BagOfCells, CellBuilder};

    #[test]
    fn reads_enwallet_v5_storage_cell() {
        let public_key = Hash32::new([0x11; 32]);
        let data_boc_base64 = data_boc(true, 3, ENWALLET_V5R1_TESTNET_WALLET_ID, public_key);
        let decoded = decode_enwallet_v5_data_boc(&data_boc_base64).expect("decode");

        assert!(decoded.is_signature_allowed);
        assert_eq!(decoded.seqno, 3);
        assert_eq!(decoded.wallet_id, ENWALLET_V5R1_TESTNET_WALLET_ID);
        assert_eq!(decoded.public_key, public_key);
        assert_eq!(decoded.extensions_count, 0);
    }

    #[test]
    fn reads_enwallet_v5_account_state_and_checks_hashes() {
        let public_key = Hash32::new([0x22; 32]);
        let data_boc_base64 = data_boc(false, 0, ENWALLET_V5R1_TESTNET_WALLET_ID, public_key);
        let data_hash =
            decode_contract_cell_boc_base64(&data_boc_base64, DEFAULT_MAX_TVM_BOC_BYTES)
                .expect("data")
                .cell_hash;
        let account = Account {
            code_hash: ENWALLET_V5R1_CODE_HASH,
            data_hash,
            storage_root: data_hash,
            data_boc_base64: Some(data_boc_base64),
            ..Account::default()
        };

        let decoded = read_enwallet_v5_state(&account).expect("state");
        assert!(!decoded.is_signature_allowed);
        assert_eq!(decoded.public_key, public_key);
    }

    fn data_boc(
        is_signature_allowed: bool,
        seqno: u32,
        wallet_id: u32,
        public_key: Hash32,
    ) -> String {
        let mut builder = CellBuilder::new();
        builder
            .store_bit(is_signature_allowed)
            .expect("signature flag")
            .store_u32(32, seqno)
            .expect("seqno")
            .store_u32(32, wallet_id)
            .expect("wallet id")
            .store_bits(256, public_key.as_bytes())
            .expect("public key")
            .store_bit(false)
            .expect("empty extensions");
        let cell = builder.build().expect("cell");
        let boc = BagOfCells::from_root(cell).serialize(false).expect("boc");
        BASE64_STANDARD.encode(boc)
    }
}

#[cfg(test)]
#[path = "enwallet/execute_tests.rs"]
mod execute_tests;
