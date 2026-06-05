use super::ApiError;
use l2_core::{
    read_enwallet_v5_state, read_sample_counter_value, Account, ENWALLET_V5R1_INTERFACE,
    ENWALLET_V5R1_LABEL,
};

pub(super) fn known_getter_result(
    method: &str,
    account: &Account,
    stack_boc: &[u8],
) -> Result<Option<serde_json::Value>, ApiError> {
    if method == "currentCounter" || method == "counter" {
        reject_known_getter_args(stack_boc)?;
        let counter = read_sample_counter_value(account)
            .map_err(|_| ApiError::bad_request("not a sample counter contract"))?;
        return Ok(Some(serde_json::json!({
                "type": "uint64",
                "value": counter.to_string(),
        })));
    }

    if matches!(
        method,
        "seqno"
            | "get_wallet_id"
            | "get_subwallet_id"
            | "get_public_key"
            | "is_signature_allowed"
            | "get_extensions"
    ) {
        reject_known_getter_args(stack_boc)?;
        let wallet = read_enwallet_v5_state(account)
            .map_err(|_| ApiError::bad_request("not an EnWallet V5 R1 contract"))?;
        let result = match method {
            "seqno" => serde_json::json!({
                "type": "uint32",
                "value": wallet.seqno.to_string(),
            }),
            "get_wallet_id" | "get_subwallet_id" => serde_json::json!({
                "type": "uint32",
                "value": wallet.wallet_id.to_string(),
            }),
            "get_public_key" => serde_json::json!({
                "type": "uint256",
                "value": wallet.public_key.to_hex(),
            }),
            "is_signature_allowed" => serde_json::json!({
                "type": "bool",
                "value": wallet.is_signature_allowed,
            }),
            "get_extensions" => serde_json::json!({
                "type": "map<uint256,bool>",
                "count": wallet.extensions_count,
            }),
            _ => unreachable!(),
        };
        return Ok(Some(serde_json::json!({
                "interface": ENWALLET_V5R1_INTERFACE,
                "interface_label": ENWALLET_V5R1_LABEL,
                "result": result,
        })));
    }

    Ok(None)
}

fn reject_known_getter_args(stack_boc: &[u8]) -> Result<(), ApiError> {
    if stack_boc.is_empty() {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            "getter parameters not supported for this method",
        ))
    }
}
