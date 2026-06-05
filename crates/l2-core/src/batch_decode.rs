use crate::crypto::Hash32;
use crate::types::{
    validate_receipt_events, L2Event, L2TransactionKind, Receipt, ReceiptStatus,
    SignedL2Transaction, MAX_RECEIPT_EVENTS,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAGIC: &[u8; 4] = b"EL2C";
const CONSENSUS_ENCODING_VERSION: u8 = 1;
const TYPE_RECEIPT: u8 = 0x02;
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DecodedBatchData {
    pub transactions: Vec<SignedL2Transaction>,
    pub receipts: Vec<Receipt>,
}

pub fn decode_batch_data(bytes: &[u8]) -> Result<DecodedBatchData, BatchDataDecodeError> {
    let mut reader = Reader::new(bytes);
    reader.read_header(TYPE_BATCH_DATA)?;
    let tx_count = reader.read_len()?;
    let mut transactions = Vec::with_capacity(tx_count);
    for _ in 0..tx_count {
        transactions.push(decode_signed_transaction(&reader.read_bytes()?)?);
    }
    let receipt_count = reader.read_len()?;
    let mut receipts = Vec::with_capacity(receipt_count);
    for _ in 0..receipt_count {
        receipts.push(decode_receipt(&reader.read_bytes()?)?);
    }
    reader.finish()?;
    Ok(DecodedBatchData {
        transactions,
        receipts,
    })
}

fn decode_signed_transaction(bytes: &[u8]) -> Result<SignedL2Transaction, BatchDataDecodeError> {
    let mut reader = Reader::new(bytes);
    reader.read_header(TYPE_SIGNED_TX)?;
    let tx_version = reader.read_u16()?;
    let domain_separator = reader.read_string()?;
    let chain_id = reader.read_string()?;
    let from = reader.read_optional_hash()?;
    let nonce = reader.read_u64()?;
    let valid_until_block = reader.read_u64()?;
    let gas_limit = reader.read_u64()?;
    let max_gas_price = reader.read_u128()?;
    let fee_asset_id = reader.read_u32()?;
    let memo_hash = reader.read_optional_hash()?;
    let transaction_kind_version = reader.read_u16()?;
    let kind = decode_transaction_kind(&mut reader)?;
    let public_key = reader.read_optional_string()?;
    let signature = reader.read_optional_string()?;
    reader.finish()?;
    Ok(SignedL2Transaction {
        tx_version,
        domain_separator,
        chain_id,
        from,
        nonce,
        valid_until_block,
        gas_limit,
        max_gas_price,
        fee_asset_id,
        memo_hash,
        transaction_kind_version,
        kind,
        public_key,
        signature,
    })
}

fn decode_transaction_kind(
    reader: &mut Reader<'_>,
) -> Result<L2TransactionKind, BatchDataDecodeError> {
    Ok(match reader.read_u8()? {
        KIND_DEPOSIT => L2TransactionKind::Deposit {
            deposit_id: reader.read_hash()?,
            asset_id: reader.read_u32()?,
            recipient: reader.read_hash()?,
            amount: reader.read_u128()?,
        },
        KIND_TRANSFER => L2TransactionKind::Transfer {
            to: reader.read_hash()?,
            asset_id: reader.read_u32()?,
            amount: reader.read_u128()?,
        },
        KIND_ROTATE_PUBLIC_KEY => L2TransactionKind::RotatePublicKey {
            new_public_key: reader.read_string()?,
        },
        KIND_WITHDRAW => L2TransactionKind::Withdraw {
            asset_id: reader.read_u32()?,
            amount: reader.read_u128()?,
            l1_recipient: reader.read_string()?,
        },
        KIND_CALL_CONTRACT => L2TransactionKind::CallContract {
            contract: reader.read_hash()?,
            body_boc_base64: reader.read_string()?,
        },
        KIND_DEPLOY_CONTRACT => L2TransactionKind::DeployContract {
            contract: reader.read_hash()?,
            code_boc_base64: reader.read_string()?,
            data_boc_base64: reader.read_string()?,
        },
        KIND_INTERNAL_MESSAGE => L2TransactionKind::InternalMessage {
            message_id: reader.read_hash()?,
            from: reader.read_hash()?,
            to: reader.read_hash()?,
            value: reader.read_u128()?,
            body_boc_base64: reader.read_string()?,
            bounce: reader.read_bool()?,
            bounced: reader.read_bool()?,
        },
        _ => return Err(BatchDataDecodeError::InvalidTag),
    })
}

fn decode_receipt(bytes: &[u8]) -> Result<Receipt, BatchDataDecodeError> {
    let mut reader = Reader::new(bytes);
    reader.read_header(TYPE_RECEIPT)?;
    let tx_hash = reader.read_hash()?;
    let status = match reader.read_u8()? {
        STATUS_APPLIED => ReceiptStatus::Applied,
        STATUS_REJECTED => ReceiptStatus::Rejected,
        _ => return Err(BatchDataDecodeError::InvalidTag),
    };
    let gas_charged = reader.read_u128()?;
    let reason = reader.read_optional_string()?;
    let withdrawal_id = reader.read_optional_hash()?;
    let events = if reader.is_finished() {
        vec![]
    } else {
        let event_count = reader.read_len()?;
        if event_count > MAX_RECEIPT_EVENTS {
            return Err(BatchDataDecodeError::TooManyEvents);
        }
        let mut events = Vec::with_capacity(event_count);
        for _ in 0..event_count {
            events.push(decode_l2_event(&mut reader)?);
        }
        validate_receipt_events(&events).map_err(|_| BatchDataDecodeError::InvalidEvent)?;
        events
    };
    reader.finish()?;
    Ok(Receipt {
        tx_hash,
        status,
        gas_charged,
        reason,
        withdrawal_id,
        events,
    })
}

fn decode_l2_event(reader: &mut Reader<'_>) -> Result<L2Event, BatchDataDecodeError> {
    Ok(match reader.read_u8()? {
        EVENT_CONTRACT_DEPLOYED => L2Event::ContractDeployed {
            contract: reader.read_hash()?,
            deployer: reader.read_hash()?,
            code_hash: reader.read_hash()?,
            data_hash: reader.read_hash()?,
        },
        EVENT_CONTRACT_CALLED => L2Event::ContractCalled {
            contract: reader.read_hash()?,
            caller: reader.read_hash()?,
            body_hash: reader.read_hash()?,
        },
        EVENT_WITHDRAWAL_CREATED => L2Event::WithdrawalCreated {
            withdrawal_id: reader.read_hash()?,
            asset_id: reader.read_u32()?,
            amount: reader.read_u128()?,
            l2_sender: reader.read_hash()?,
            l1_recipient: reader.read_string()?,
        },
        _ => return Err(BatchDataDecodeError::InvalidTag),
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_header(&mut self, expected_type: u8) -> Result<(), BatchDataDecodeError> {
        if self.read_exact(MAGIC.len())? != MAGIC {
            return Err(BatchDataDecodeError::InvalidMagic);
        }
        if self.read_u8()? != CONSENSUS_ENCODING_VERSION {
            return Err(BatchDataDecodeError::UnsupportedVersion);
        }
        if self.read_u8()? != expected_type {
            return Err(BatchDataDecodeError::WrongType);
        }
        Ok(())
    }

    fn finish(&self) -> Result<(), BatchDataDecodeError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(BatchDataDecodeError::TrailingBytes)
        }
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn read_optional_hash(&mut self) -> Result<Option<Hash32>, BatchDataDecodeError> {
        match self.read_u8()? {
            0 => Ok(None),
            1 => Ok(Some(self.read_hash()?)),
            _ => Err(BatchDataDecodeError::InvalidOption),
        }
    }

    fn read_optional_string(&mut self) -> Result<Option<String>, BatchDataDecodeError> {
        match self.read_u8()? {
            0 => Ok(None),
            1 => Ok(Some(self.read_string()?)),
            _ => Err(BatchDataDecodeError::InvalidOption),
        }
    }

    fn read_string(&mut self) -> Result<String, BatchDataDecodeError> {
        String::from_utf8(self.read_bytes()?).map_err(|_| BatchDataDecodeError::InvalidUtf8)
    }

    fn read_bytes(&mut self) -> Result<Vec<u8>, BatchDataDecodeError> {
        let len = self.read_len()?;
        Ok(self.read_exact(len)?.to_vec())
    }

    fn read_len(&mut self) -> Result<usize, BatchDataDecodeError> {
        usize::try_from(self.read_u32()?).map_err(|_| BatchDataDecodeError::LengthOverflow)
    }

    fn read_hash(&mut self) -> Result<Hash32, BatchDataDecodeError> {
        let bytes: [u8; 32] = self
            .read_exact(32)?
            .try_into()
            .expect("fixed slice length is 32");
        Ok(Hash32::new(bytes))
    }

    fn read_u8(&mut self) -> Result<u8, BatchDataDecodeError> {
        Ok(*self
            .read_exact(1)?
            .first()
            .expect("fixed slice length is 1"))
    }

    fn read_bool(&mut self) -> Result<bool, BatchDataDecodeError> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(BatchDataDecodeError::InvalidTag),
        }
    }

    fn read_u32(&mut self) -> Result<u32, BatchDataDecodeError> {
        Ok(u32::from_be_bytes(
            self.read_exact(4)?
                .try_into()
                .expect("fixed slice length is 4"),
        ))
    }

    fn read_u16(&mut self) -> Result<u16, BatchDataDecodeError> {
        Ok(u16::from_be_bytes(
            self.read_exact(2)?
                .try_into()
                .expect("fixed slice length is 2"),
        ))
    }

    fn read_u64(&mut self) -> Result<u64, BatchDataDecodeError> {
        Ok(u64::from_be_bytes(
            self.read_exact(8)?
                .try_into()
                .expect("fixed slice length is 8"),
        ))
    }

    fn read_u128(&mut self) -> Result<u128, BatchDataDecodeError> {
        Ok(u128::from_be_bytes(
            self.read_exact(16)?
                .try_into()
                .expect("fixed slice length is 16"),
        ))
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], BatchDataDecodeError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(BatchDataDecodeError::LengthOverflow)?;
        if end > self.bytes.len() {
            return Err(BatchDataDecodeError::UnexpectedEof);
        }
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum BatchDataDecodeError {
    #[error("batch data has invalid magic")]
    InvalidMagic,
    #[error("batch data uses unsupported consensus version")]
    UnsupportedVersion,
    #[error("batch data has unexpected type tag")]
    WrongType,
    #[error("batch data ended unexpectedly")]
    UnexpectedEof,
    #[error("batch data contains trailing bytes")]
    TrailingBytes,
    #[error("batch data length overflows platform usize")]
    LengthOverflow,
    #[error("batch data contains invalid option tag")]
    InvalidOption,
    #[error("batch data contains invalid enum tag")]
    InvalidTag,
    #[error("batch data contains invalid UTF-8")]
    InvalidUtf8,
    #[error("batch data receipt contains too many events")]
    TooManyEvents,
    #[error("batch data receipt contains an invalid event")]
    InvalidEvent,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::{encode_batch_data, encode_receipt, encode_signed_transaction};
    use crate::crypto::sha256_bytes;

    #[test]
    fn batch_data_decode_roundtrips_canonical_bytes() {
        let tx = SignedL2Transaction::system_deposit(
            "entropis-testnet",
            sha256_bytes(b"deposit"),
            0,
            sha256_bytes(b"recipient"),
            100,
        );
        let receipt = Receipt::applied(tx.tx_hash(), 0, None);
        let decoded = decode_batch_data(&encode_batch_data(
            std::slice::from_ref(&tx),
            std::slice::from_ref(&receipt),
        ))
        .expect("decode");

        assert_eq!(decoded.transactions, vec![tx]);
        assert_eq!(decoded.receipts, vec![receipt]);
    }

    #[test]
    fn batch_data_decode_accepts_pre_event_receipts_as_empty_events() {
        let tx = SignedL2Transaction::system_deposit(
            "entropis-testnet",
            sha256_bytes(b"deposit"),
            0,
            sha256_bytes(b"recipient"),
            100,
        );
        let receipt = Receipt::applied(tx.tx_hash(), 0, None);
        let mut old_receipt = encode_receipt(&receipt);
        old_receipt.truncate(old_receipt.len() - 4);

        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.push(CONSENSUS_ENCODING_VERSION);
        bytes.push(TYPE_BATCH_DATA);
        push_u32(&mut bytes, 1);
        push_bytes(&mut bytes, &encode_signed_transaction(&tx));
        push_u32(&mut bytes, 1);
        push_bytes(&mut bytes, &old_receipt);

        let decoded = decode_batch_data(&bytes).expect("decode old receipt");
        assert_eq!(decoded.receipts, vec![receipt]);
    }

    #[test]
    fn batch_data_decode_rejects_bad_magic_and_trailing_bytes() {
        let tx = SignedL2Transaction::system_deposit(
            "entropis-testnet",
            sha256_bytes(b"deposit"),
            0,
            sha256_bytes(b"recipient"),
            100,
        );
        let receipt = Receipt::applied(tx.tx_hash(), 0, None);
        let mut bytes = encode_batch_data(&[tx], &[receipt]);

        let mut bad_magic = bytes.clone();
        bad_magic[0] = 0;
        assert_eq!(
            decode_batch_data(&bad_magic).unwrap_err(),
            BatchDataDecodeError::InvalidMagic
        );

        bytes.push(0);
        assert_eq!(
            decode_batch_data(&bytes).unwrap_err(),
            BatchDataDecodeError::TrailingBytes
        );
    }

    fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
        push_u32(out, bytes.len() as u32);
        out.extend_from_slice(bytes);
    }

    fn push_u32(out: &mut Vec<u8>, value: u32) {
        out.extend_from_slice(&value.to_be_bytes());
    }
}
