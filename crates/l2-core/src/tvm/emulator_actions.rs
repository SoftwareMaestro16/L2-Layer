use crate::crypto::Hash32;
use crate::tvm::{TvmAdapterError, TvmInternalMessage};
use num_bigint::BigUint;
use tonlib_core::cell::BagOfCells;
use tonlib_core::tlb_types::block::message::{CommonMsgInfo, Message};
use tonlib_core::tlb_types::block::msg_address::{MsgAddress, MsgAddressExt, MsgAddressInt};
use tonlib_core::tlb_types::block::out_action::{OutAction, OutList};
use tonlib_core::tlb_types::tlb::TLB;

pub(super) fn parse_actions(
    actions_boc_base64: Option<&str>,
    contract: Hash32,
    workchain: i32,
    max_messages: u32,
) -> Result<Vec<TvmInternalMessage>, TvmAdapterError> {
    let Some(actions_boc_base64) = actions_boc_base64.filter(|value| !value.is_empty()) else {
        return Ok(vec![]);
    };
    let out_list =
        OutList::from_boc_b64(actions_boc_base64).map_err(|_| TvmAdapterError::Rejected {
            reason: "tvm_malformed_actions",
        })?;
    let mut messages = Vec::new();
    collect_actions(&out_list, contract, workchain, max_messages, &mut messages)?;
    Ok(messages)
}

fn collect_actions(
    out_list: &OutList,
    contract: Hash32,
    workchain: i32,
    max_messages: u32,
    out: &mut Vec<TvmInternalMessage>,
) -> Result<(), TvmAdapterError> {
    match out_list {
        OutList::Empty => Ok(()),
        OutList::Some(node) => {
            if out.len() >= max_messages as usize {
                return Err(TvmAdapterError::Rejected {
                    reason: "too_many_internal_messages",
                });
            }
            match &node.action {
                OutAction::SendMsg(action) => {
                    let message = Message::from_cell(action.out_msg.as_ref()).map_err(|_| {
                        TvmAdapterError::Rejected {
                            reason: "tvm_malformed_out_message",
                        }
                    })?;
                    out.push(convert_internal_message(message, contract, workchain)?);
                }
                _ => {
                    return Err(TvmAdapterError::Rejected {
                        reason: "tvm_unsupported_action",
                    });
                }
            }
            let prev = OutList::from_cell(node.prev.0.as_ref()).map_err(|_| {
                TvmAdapterError::Rejected {
                    reason: "tvm_malformed_actions",
                }
            })?;
            collect_actions(&prev, contract, workchain, max_messages, out)
        }
    }
}

fn convert_internal_message(
    message: Message,
    contract: Hash32,
    workchain: i32,
) -> Result<TvmInternalMessage, TvmAdapterError> {
    let CommonMsgInfo::Int(info) = message.info else {
        return Err(TvmAdapterError::Rejected {
            reason: "tvm_unsupported_out_message",
        });
    };
    if let Some(source) = optional_l2_address(&info.src, workchain)? {
        if source != contract {
            return Err(TvmAdapterError::Rejected {
                reason: "tvm_out_message_source_mismatch",
            });
        }
    }
    if info.value.other.is_some() {
        return Err(TvmAdapterError::Rejected {
            reason: "tvm_unsupported_currency",
        });
    }
    let body_boc = BagOfCells::from_root(message.body.value.as_ref().clone())
        .serialize(false)
        .map_err(|_| TvmAdapterError::Rejected {
            reason: "tvm_malformed_out_message",
        })?;
    Ok(TvmInternalMessage {
        from: contract,
        to: required_l2_address(&info.dest, workchain)?,
        value: biguint_to_u128(&info.value.grams.amount)?,
        body_boc,
    })
}

fn required_l2_address(address: &MsgAddress, workchain: i32) -> Result<Hash32, TvmAdapterError> {
    optional_l2_address(address, workchain)?.ok_or(TvmAdapterError::Rejected {
        reason: "tvm_unsupported_out_message",
    })
}

fn optional_l2_address(
    address: &MsgAddress,
    workchain: i32,
) -> Result<Option<Hash32>, TvmAdapterError> {
    match address {
        MsgAddress::Int(MsgAddressInt::Std(address)) => {
            if address.anycast.is_some() || address.workchain != workchain {
                return Err(TvmAdapterError::Rejected {
                    reason: "tvm_unsupported_out_message",
                });
            }
            hash_from_slice(&address.address)
                .map(Some)
                .map_err(|_| TvmAdapterError::Rejected {
                    reason: "tvm_unsupported_out_message",
                })
        }
        MsgAddress::Ext(MsgAddressExt::None(_)) => Ok(None),
        _ => Err(TvmAdapterError::Rejected {
            reason: "tvm_unsupported_out_message",
        }),
    }
}

fn biguint_to_u128(value: &BigUint) -> Result<u128, TvmAdapterError> {
    let bytes = value.to_bytes_be();
    if bytes.len() > 16 {
        return Err(TvmAdapterError::Rejected {
            reason: "tvm_value_overflow",
        });
    }
    let mut out = [0u8; 16];
    out[16 - bytes.len()..].copy_from_slice(&bytes);
    Ok(u128::from_be_bytes(out))
}

fn hash_from_slice(value: &[u8]) -> Result<Hash32, ()> {
    let bytes: [u8; 32] = value.try_into().map_err(|_| ())?;
    Ok(Hash32::new(bytes))
}
