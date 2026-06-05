use l2_core::{
    decode_contract_cell_boc_base64, Account, Hash32, DEFAULT_MAX_CONTRACT_CODE_BOC_BYTES,
    DEFAULT_MAX_CONTRACT_DATA_BOC_BYTES,
};
use serde::{Deserialize, Serialize};

use super::StorageError;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoredContractCodeCell {
    pub code_hash: Hash32,
    pub code_boc_base64: String,
    pub size_bytes: usize,
    pub first_seen_block_height: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoredContractDataCell {
    pub data_hash: Hash32,
    pub storage_root: Hash32,
    pub data_boc_base64: String,
    pub size_bytes: usize,
    pub first_seen_block_height: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoredContractState {
    pub account_id: Hash32,
    pub account: Account,
    pub code_cell: StoredContractCodeCell,
    pub data_cell: StoredContractDataCell,
    pub last_block_height: u64,
}

impl StoredContractState {
    pub fn from_account(
        account_id: Hash32,
        account: &Account,
        block_height: u64,
    ) -> Result<Option<Self>, StorageError> {
        if account.code_hash == Hash32::ZERO
            && account.data_hash == Hash32::ZERO
            && account.storage_root == Hash32::ZERO
            && account.code_boc_base64.is_none()
            && account.data_boc_base64.is_none()
        {
            return Ok(None);
        }

        let code_boc_base64 =
            account
                .code_boc_base64
                .as_deref()
                .ok_or(StorageError::MissingContractCell {
                    field: "code_boc_base64",
                })?;
        let data_boc_base64 =
            account
                .data_boc_base64
                .as_deref()
                .ok_or(StorageError::MissingContractCell {
                    field: "data_boc_base64",
                })?;
        let code_cell =
            decode_contract_cell_boc_base64(code_boc_base64, DEFAULT_MAX_CONTRACT_CODE_BOC_BYTES)
                .map_err(|error| invalid_contract_cell("code_boc_base64", error))?;
        let data_cell =
            decode_contract_cell_boc_base64(data_boc_base64, DEFAULT_MAX_CONTRACT_DATA_BOC_BYTES)
                .map_err(|error| invalid_contract_cell("data_boc_base64", error))?;

        if code_cell.cell_hash != account.code_hash {
            return Err(StorageError::ContractCellHashMismatch {
                field: "code_boc_base64",
                expected: account.code_hash,
                actual: code_cell.cell_hash,
            });
        }
        if data_cell.cell_hash != account.data_hash {
            return Err(StorageError::ContractCellHashMismatch {
                field: "data_boc_base64",
                expected: account.data_hash,
                actual: data_cell.cell_hash,
            });
        }

        Ok(Some(Self {
            account_id,
            account: account.clone(),
            code_cell: StoredContractCodeCell {
                code_hash: code_cell.cell_hash,
                code_boc_base64: code_cell.boc_base64,
                size_bytes: code_cell.boc_bytes.len(),
                first_seen_block_height: block_height,
            },
            data_cell: StoredContractDataCell {
                data_hash: data_cell.cell_hash,
                storage_root: account.storage_root,
                data_boc_base64: data_cell.boc_base64,
                size_bytes: data_cell.boc_bytes.len(),
                first_seen_block_height: block_height,
            },
            last_block_height: block_height,
        }))
    }
}

fn invalid_contract_cell(field: &'static str, error: impl ToString) -> StorageError {
    StorageError::InvalidContractCell {
        field,
        reason: error.to_string(),
    }
}
