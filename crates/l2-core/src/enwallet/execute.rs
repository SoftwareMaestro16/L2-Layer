use super::{decode_enwallet_v5_data_boc, EnWalletReadError, ENWALLET_V5R1_CODE_HASH};
use crate::crypto::Hash32;
use crate::tvm::{
    boc_single_root_hash, decode_contract_cell_boc_base64, TvmAdapterError, TvmExecutionInput,
    TvmExecutionOutput, TvmExecutionStatus, TvmStateDelta, DEFAULT_MAX_TVM_BOC_BYTES,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use tonlib_core::cell::{BagOfCells, CellBuilder};

const EXTERNAL_SIGNED_REQUEST: u32 = 0x7369_676e;
const INTERNAL_SIGNED_REQUEST: u32 = 0x7369_6e74;
const SIGNED_NO_ACTION_GAS: u64 = 25;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SignedRequest {
    opcode: u32,
    wallet_id: u32,
    valid_until: u32,
    seqno: u32,
    signature: [u8; 64],
}

pub fn execute_enwallet_v5r1(
    input: &TvmExecutionInput,
) -> Result<Option<TvmExecutionOutput>, TvmAdapterError> {
    if input.contract_state.code_hash != ENWALLET_V5R1_CODE_HASH {
        return Ok(None);
    }
    if input.gas_limit < SIGNED_NO_ACTION_GAS {
        return Ok(Some(TvmExecutionOutput::rejected(
            input.gas_limit.max(1),
            "gas_exhausted",
        )));
    }

    let state = match decode_state(input) {
        Ok(state) => state,
        Err(reason) => return Ok(Some(rejected(reason))),
    };
    if state.extensions_count != 0 {
        return Ok(Some(rejected("enwallet_extensions_unsupported")));
    }

    let request = match decode_signed_request(&input.input_boc) {
        Ok(request) => request,
        Err(reason) => return Ok(Some(rejected(reason))),
    };
    if request.wallet_id != state.wallet_id {
        return Ok(Some(rejected("enwallet_invalid_wallet_id")));
    }
    if request.seqno != state.seqno {
        return Ok(Some(rejected("enwallet_invalid_seqno")));
    }
    if u64::from(request.valid_until) <= input.context.block_time {
        return Ok(Some(rejected("enwallet_expired")));
    }
    if !state.is_signature_allowed && state.extensions_count != 0 {
        return Ok(Some(rejected("enwallet_signature_disabled")));
    }
    if !verify_signed_request(&request, state.public_key) {
        return Ok(Some(rejected("enwallet_invalid_signature")));
    }

    let next_seqno = request
        .seqno
        .checked_add(1)
        .ok_or(TvmAdapterError::Rejected {
            reason: "enwallet_seqno_overflow",
        })?;
    let data_boc_base64 = encode_data_boc(next_seqno, state.wallet_id, state.public_key)?;
    let data_hash = cell_hash(&data_boc_base64)?;

    Ok(Some(TvmExecutionOutput {
        status: TvmExecutionStatus::Applied,
        state_delta: Some(TvmStateDelta {
            contract: input.contract,
            code_hash: None,
            code_boc_base64: None,
            data_hash: Some(data_hash),
            data_boc_base64: Some(data_boc_base64),
            storage_root: Some(data_hash),
        }),
        emitted_internal_messages: vec![],
        gas_used: SIGNED_NO_ACTION_GAS,
    }))
}

fn decode_state(input: &TvmExecutionInput) -> Result<super::EnWalletV5State, &'static str> {
    if input.contract_state.data_hash != input.contract_state.storage_root {
        return Err("enwallet_state_hash_mismatch");
    }
    let data_boc_base64 = input
        .contract_state
        .data_boc_base64
        .as_deref()
        .ok_or("enwallet_data_missing")?;
    let data_cell = decode_contract_cell_boc_base64(data_boc_base64, DEFAULT_MAX_TVM_BOC_BYTES)
        .map_err(|_| "enwallet_malformed_data")?;
    if data_cell.cell_hash != input.contract_state.data_hash {
        return Err("enwallet_state_hash_mismatch");
    }
    decode_enwallet_v5_data_boc(&data_cell.boc_base64).map_err(read_error_reason)
}

fn read_error_reason(error: EnWalletReadError) -> &'static str {
    match error {
        EnWalletReadError::UnsupportedCodeHash => "enwallet_unsupported_code",
        EnWalletReadError::MissingDataBoc => "enwallet_data_missing",
        EnWalletReadError::MalformedDataBoc => "enwallet_malformed_data",
        EnWalletReadError::DataHashMismatch => "enwallet_state_hash_mismatch",
    }
}

fn decode_signed_request(input_boc: &[u8]) -> Result<SignedRequest, &'static str> {
    let root = BagOfCells::parse(input_boc)
        .and_then(BagOfCells::single_root)
        .map_err(|_| "enwallet_malformed_body")?;
    let mut parser = root.parser();
    let opcode = parser.load_u32(32).map_err(|_| "enwallet_malformed_body")?;
    if opcode != EXTERNAL_SIGNED_REQUEST && opcode != INTERNAL_SIGNED_REQUEST {
        return Err("enwallet_bad_opcode");
    }
    let wallet_id = parser.load_u32(32).map_err(|_| "enwallet_malformed_body")?;
    let valid_until = parser.load_u32(32).map_err(|_| "enwallet_malformed_body")?;
    let seqno = parser.load_u32(32).map_err(|_| "enwallet_malformed_body")?;
    if parser.load_bit().map_err(|_| "enwallet_malformed_body")? {
        return Err("enwallet_c5_actions_unsupported");
    }
    if parser.load_bit().map_err(|_| "enwallet_malformed_body")? {
        return Err("enwallet_extra_actions_unsupported");
    }
    let signature = parser
        .load_bits(512)
        .map_err(|_| "enwallet_malformed_body")?
        .try_into()
        .map_err(|_| "enwallet_malformed_body")?;
    if parser.remaining_bits() != 0 || parser.remaining_refs() != 0 {
        return Err("enwallet_malformed_body");
    }
    Ok(SignedRequest {
        opcode,
        wallet_id,
        valid_until,
        seqno,
        signature,
    })
}

fn verify_signed_request(request: &SignedRequest, public_key: Hash32) -> bool {
    let Ok(unsigned_hash) = unsigned_request_hash(request) else {
        return false;
    };
    let Ok(verifying_key) = VerifyingKey::from_bytes(public_key.as_bytes()) else {
        return false;
    };
    let signature = Signature::from_bytes(&request.signature);
    verifying_key
        .verify(unsigned_hash.as_bytes(), &signature)
        .is_ok()
}

fn unsigned_request_hash(request: &SignedRequest) -> Result<Hash32, TvmAdapterError> {
    let unsigned = request_cell(request, false)?;
    let boc = BagOfCells::from_root(unsigned)
        .serialize(false)
        .map_err(|_| TvmAdapterError::Rejected {
            reason: "enwallet_malformed_body",
        })?;
    boc_single_root_hash(&boc).map_err(|_| TvmAdapterError::Rejected {
        reason: "enwallet_malformed_body",
    })
}

fn request_cell(
    request: &SignedRequest,
    include_signature: bool,
) -> Result<tonlib_core::cell::Cell, TvmAdapterError> {
    let mut builder = CellBuilder::new();
    builder
        .store_u32(32, request.opcode)
        .and_then(|builder| builder.store_u32(32, request.wallet_id))
        .and_then(|builder| builder.store_u32(32, request.valid_until))
        .and_then(|builder| builder.store_u32(32, request.seqno))
        .and_then(|builder| builder.store_bit(false))
        .and_then(|builder| builder.store_bit(false))
        .map_err(|_| TvmAdapterError::Rejected {
            reason: "enwallet_malformed_body",
        })?;
    if include_signature {
        builder
            .store_bits(512, &request.signature)
            .map_err(|_| TvmAdapterError::Rejected {
                reason: "enwallet_malformed_body",
            })?;
    }
    builder.build().map_err(|_| TvmAdapterError::Rejected {
        reason: "enwallet_malformed_body",
    })
}

fn encode_data_boc(
    seqno: u32,
    wallet_id: u32,
    public_key: Hash32,
) -> Result<String, TvmAdapterError> {
    let mut builder = CellBuilder::new();
    builder
        .store_bit(true)
        .and_then(|builder| builder.store_u32(32, seqno))
        .and_then(|builder| builder.store_u32(32, wallet_id))
        .and_then(|builder| builder.store_bits(256, public_key.as_bytes()))
        .and_then(|builder| builder.store_bit(false))
        .map_err(|_| TvmAdapterError::Rejected {
            reason: "enwallet_malformed_data",
        })?;
    let cell = builder.build().map_err(|_| TvmAdapterError::Rejected {
        reason: "enwallet_malformed_data",
    })?;
    let boc =
        BagOfCells::from_root(cell)
            .serialize(false)
            .map_err(|_| TvmAdapterError::Rejected {
                reason: "enwallet_malformed_data",
            })?;
    Ok(BASE64_STANDARD.encode(boc))
}

fn cell_hash(boc_base64: &str) -> Result<Hash32, TvmAdapterError> {
    let boc =
        BASE64_STANDARD
            .decode(boc_base64.as_bytes())
            .map_err(|_| TvmAdapterError::Rejected {
                reason: "enwallet_malformed_data",
            })?;
    boc_single_root_hash(&boc).map_err(|_| TvmAdapterError::Rejected {
        reason: "enwallet_malformed_data",
    })
}

fn rejected(reason: &'static str) -> TvmExecutionOutput {
    TvmExecutionOutput::rejected(SIGNED_NO_ACTION_GAS, reason)
}
