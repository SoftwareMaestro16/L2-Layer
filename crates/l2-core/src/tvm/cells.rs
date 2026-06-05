use crate::crypto::Hash32;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use tonlib_core::cell::{BagOfCells, TonCellError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractCell {
    pub boc_base64: String,
    pub boc_bytes: Vec<u8>,
    pub cell_hash: Hash32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ContractCellError {
    #[error("BoC exceeds max size")]
    BocTooLarge,
    #[error("BoC is malformed")]
    MalformedBoc,
}

impl ContractCellError {
    pub fn deploy_reason(self, field: ContractCellField) -> &'static str {
        match (field, self) {
            (ContractCellField::Code, Self::BocTooLarge) => "code_boc_too_large",
            (ContractCellField::Code, Self::MalformedBoc) => "malformed_code_boc",
            (ContractCellField::Data, Self::BocTooLarge) => "data_boc_too_large",
            (ContractCellField::Data, Self::MalformedBoc) => "malformed_data_boc",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractCellField {
    Code,
    Data,
}

pub fn decode_contract_cell_boc_base64(
    boc_base64: &str,
    max_boc_bytes: usize,
) -> Result<ContractCell, ContractCellError> {
    let max_encoded_bytes = max_boc_bytes
        .saturating_add(2)
        .saturating_div(3)
        .saturating_mul(4);
    if boc_base64.is_empty() || boc_base64.len() > max_encoded_bytes {
        return Err(if boc_base64.is_empty() {
            ContractCellError::MalformedBoc
        } else {
            ContractCellError::BocTooLarge
        });
    }

    let boc_bytes = BASE64_STANDARD
        .decode(boc_base64.as_bytes())
        .map_err(|_| ContractCellError::MalformedBoc)?;
    if boc_bytes.is_empty() || boc_bytes.len() > max_boc_bytes {
        return Err(if boc_bytes.is_empty() {
            ContractCellError::MalformedBoc
        } else {
            ContractCellError::BocTooLarge
        });
    }
    let cell_hash =
        boc_single_root_hash(&boc_bytes).map_err(|_| ContractCellError::MalformedBoc)?;
    Ok(ContractCell {
        boc_base64: BASE64_STANDARD.encode(&boc_bytes),
        boc_bytes,
        cell_hash,
    })
}

pub fn boc_single_root_hash(boc: &[u8]) -> Result<Hash32, TonCellError> {
    let root = BagOfCells::parse(boc)?.single_root()?;
    let bytes: [u8; 32] = root
        .cell_hash()
        .as_slice()
        .try_into()
        .expect("TON cell hash is always 32 bytes");
    Ok(Hash32::new(bytes))
}
