use super::IndexerError;
use base64::prelude::{Engine as _, BASE64_STANDARD, BASE64_URL_SAFE_NO_PAD};
use l2_core::Hash32;
use serde_json::Value;
use tonlib_core::types::TonAddress;

pub(super) fn parse_opcode(value: Option<&Value>) -> Result<u32, IndexerError> {
    match value {
        Some(Value::Number(number)) => number
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or(IndexerError::Decode("opcode is not uint32")),
        Some(Value::String(value)) => {
            let value = value.trim();
            if let Some(hex) = value.strip_prefix("0x") {
                u32::from_str_radix(hex, 16).map_err(|_| IndexerError::Decode("bad opcode hex"))
            } else {
                value
                    .parse::<u32>()
                    .map_err(|_| IndexerError::Decode("bad opcode"))
            }
        }
        _ => Err(IndexerError::Decode("opcode is missing")),
    }
}

pub(super) fn field<'a>(value: &'a Value, names: &[&str]) -> Result<&'a Value, IndexerError> {
    let object = value
        .as_object()
        .ok_or(IndexerError::Decode("decoded payload is not an object"))?;
    names
        .iter()
        .find_map(|name| object.get(*name))
        .ok_or(IndexerError::Decode("decoded payload field is missing"))
}

pub(super) fn parse_u64_value(
    value: Option<&Value>,
    field: &'static str,
) -> Result<u64, IndexerError> {
    let value = value.ok_or(IndexerError::Decode(field))?;
    match value {
        Value::Number(number) => number.as_u64().ok_or(IndexerError::Decode(field)),
        Value::String(value) => value
            .parse::<u64>()
            .map_err(|_| IndexerError::Decode(field)),
        _ => Err(IndexerError::Decode(field)),
    }
}

pub(super) fn parse_u32_value(value: &Value, field: &'static str) -> Result<u32, IndexerError> {
    let value = parse_u64_value(Some(value), field)?;
    u32::try_from(value).map_err(|_| IndexerError::Decode(field))
}

pub(super) fn parse_u128_value(value: &Value, field: &'static str) -> Result<u128, IndexerError> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .map(u128::from)
            .ok_or(IndexerError::Decode(field)),
        Value::String(value) => value
            .parse::<u128>()
            .map_err(|_| IndexerError::Decode(field)),
        _ => Err(IndexerError::Decode(field)),
    }
}

pub(super) fn parse_uint256_hash(value: &Value) -> Result<Hash32, IndexerError> {
    match value {
        Value::String(value) => parse_hash_or_decimal(value),
        Value::Number(number) => {
            let value = number
                .as_u64()
                .ok_or(IndexerError::Decode("uint256 number is invalid"))?;
            Ok(uint256_decimal_to_hash(&value.to_string())?)
        }
        _ => Err(IndexerError::Decode("uint256 field is invalid")),
    }
}

pub(super) fn parse_message_hash(value: Option<&String>) -> Result<Hash32, IndexerError> {
    let value = value.ok_or(IndexerError::Decode("message hash is missing"))?;
    parse_hash_or_base64(value)
}

pub(super) fn ton_addresses_match(left: &str, right: &str) -> bool {
    match (canonical_ton_address(left), canonical_ton_address(right)) {
        (Some(left), Some(right)) => left == right,
        _ => left == right,
    }
}

fn canonical_ton_address(value: &str) -> Option<String> {
    TonAddress::from_base64_url(value)
        .or_else(|_| TonAddress::from_base64_std(value))
        .or_else(|_| TonAddress::from_hex_str(value))
        .ok()
        .map(|address| address.to_hex())
}

pub(super) fn hash32_from_tonhash(value: tonlib_core::types::TonHash) -> Hash32 {
    let bytes: [u8; 32] = value
        .as_slice()
        .try_into()
        .expect("TonHash is always 32 bytes");
    Hash32::new(bytes)
}

pub(super) fn biguint_to_u128(value: num_bigint::BigUint) -> Result<u128, IndexerError> {
    let bytes = value.to_bytes_be();
    if bytes.len() > 16 {
        return Err(IndexerError::Decode("amount exceeds u128"));
    }
    let mut out = [0u8; 16];
    out[16 - bytes.len()..].copy_from_slice(&bytes);
    Ok(u128::from_be_bytes(out))
}

fn parse_hash_or_decimal(value: &str) -> Result<Hash32, IndexerError> {
    let value = value.trim();
    if value.starts_with("0x")
        || value.len() == 64 && value.chars().all(|char| char.is_ascii_hexdigit())
    {
        return Hash32::from_hex(value).map_err(|_| IndexerError::Decode("bad uint256 hex"));
    }
    uint256_decimal_to_hash(value)
}

pub(super) fn parse_hash_or_base64(value: &str) -> Result<Hash32, IndexerError> {
    let value = value.trim();
    if value.starts_with("0x")
        || value.len() == 64 && value.chars().all(|char| char.is_ascii_hexdigit())
    {
        return Hash32::from_hex(value).map_err(|_| IndexerError::Decode("bad hash hex"));
    }

    let decoded = BASE64_STANDARD
        .decode(value)
        .or_else(|_| BASE64_URL_SAFE_NO_PAD.decode(value))
        .map_err(|_| IndexerError::Decode("bad hash encoding"))?;
    let bytes: [u8; 32] = decoded
        .try_into()
        .map_err(|_| IndexerError::Decode("hash must be 32 bytes"))?;
    Ok(Hash32::new(bytes))
}

fn uint256_decimal_to_hash(value: &str) -> Result<Hash32, IndexerError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(IndexerError::Decode("bad uint256 decimal"));
    }
    let mut out = [0u8; 32];
    for digit in value.bytes().map(|byte| byte - b'0') {
        let mut carry = u16::from(digit);
        for byte in out.iter_mut().rev() {
            let next = u16::from(*byte) * 10 + carry;
            *byte = next as u8;
            carry = next >> 8;
        }
        if carry != 0 {
            return Err(IndexerError::Decode("uint256 decimal overflow"));
        }
    }
    Ok(Hash32::new(out))
}
