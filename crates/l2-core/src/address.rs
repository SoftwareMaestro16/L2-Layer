use crate::crypto::Hash32;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use thiserror::Error;

pub const L2_RAW_ADDRESS_PREFIX: &str = "8:";
pub const L2_USER_FRIENDLY_TAG: u8 = 0x11;
pub const L2_USER_FRIENDLY_NETWORK: u8 = 0x78;
pub const L2_USER_FRIENDLY_LEN: usize = 48;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum L2AddressError {
    #[error("invalid l2 address")]
    Invalid,
    #[error("invalid l2 address checksum")]
    InvalidChecksum,
}

pub fn l2_raw_address(account_id: Hash32) -> String {
    format!("{L2_RAW_ADDRESS_PREFIX}{}", account_id.to_hex())
}

pub fn l2_user_friendly_address(account_id: Hash32) -> String {
    let mut payload = [0u8; 36];
    payload[0] = L2_USER_FRIENDLY_TAG;
    payload[1] = L2_USER_FRIENDLY_NETWORK;
    payload[2..34].copy_from_slice(account_id.as_bytes());
    let checksum = crc16_xmodem(&payload[..34]);
    payload[34] = (checksum >> 8) as u8;
    payload[35] = checksum as u8;
    URL_SAFE_NO_PAD.encode(payload)
}

pub fn parse_l2_address(value: &str) -> Result<Hash32, L2AddressError> {
    if let Some(hex) = value.strip_prefix(L2_RAW_ADDRESS_PREFIX) {
        return parse_hash32_hex(hex);
    }
    if value.len() == L2_USER_FRIENDLY_LEN && value.starts_with("EX") {
        return parse_user_friendly(value);
    }
    parse_hash32_hex(value.strip_prefix("0x").unwrap_or(value))
}

fn parse_hash32_hex(value: &str) -> Result<Hash32, L2AddressError> {
    Hash32::from_hex(value).map_err(|_| L2AddressError::Invalid)
}

fn parse_user_friendly(value: &str) -> Result<Hash32, L2AddressError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| L2AddressError::Invalid)?;
    if bytes.len() != 36 || bytes[0] != L2_USER_FRIENDLY_TAG || bytes[1] != L2_USER_FRIENDLY_NETWORK
    {
        return Err(L2AddressError::Invalid);
    }

    let expected = crc16_xmodem(&bytes[..34]);
    let actual = u16::from_be_bytes([bytes[34], bytes[35]]);
    if expected != actual {
        return Err(L2AddressError::InvalidChecksum);
    }

    let mut account_id = [0u8; 32];
    account_id.copy_from_slice(&bytes[2..34]);
    Ok(Hash32::new(account_id))
}

fn crc16_xmodem(bytes: &[u8]) -> u16 {
    let mut crc = 0u16;
    for byte in bytes {
        crc ^= u16::from(*byte) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::sha256_bytes;
    use std::collections::BTreeSet;

    #[test]
    fn raw_l2_address_roundtrips_with_8_prefix() {
        let account_id = sha256_bytes(b"entropis-account");
        let raw = l2_raw_address(account_id);

        assert_eq!(raw.len(), 66);
        assert!(raw.starts_with("8:"));
        assert_eq!(parse_l2_address(&raw), Ok(account_id));
    }

    #[test]
    fn user_friendly_l2_address_roundtrips_with_ex_prefix() {
        let account_id = sha256_bytes(b"entropis-account");
        let friendly = l2_user_friendly_address(account_id);

        assert_eq!(friendly.len(), L2_USER_FRIENDLY_LEN);
        assert!(friendly.starts_with("EX"));
        assert_eq!(parse_l2_address(&friendly), Ok(account_id));
    }

    #[test]
    fn user_friendly_addresses_can_use_the_full_base64url_alphabet_after_ex() {
        let mut observed = BTreeSet::new();
        for index in 0..10_000 {
            let account_id = sha256_bytes(format!("entropis-{index}").as_bytes());
            let friendly = l2_user_friendly_address(account_id);
            observed.extend(friendly[2..].chars());
        }

        let expected = "-0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz"
            .chars()
            .collect::<BTreeSet<_>>();
        assert_eq!(observed, expected);
    }

    #[test]
    fn legacy_hex_is_accepted_for_internal_compatibility() {
        let account_id = sha256_bytes(b"entropis-account");

        assert_eq!(parse_l2_address(&account_id.to_hex()), Ok(account_id));
        assert_eq!(
            parse_l2_address(&format!("0x{}", account_id.to_hex())),
            Ok(account_id)
        );
    }

    #[test]
    fn user_friendly_checksum_is_checked() {
        let mut friendly = l2_user_friendly_address(sha256_bytes(b"entropis-account"));
        let replacement = if friendly.ends_with('A') { "B" } else { "A" };
        friendly.replace_range(47..48, replacement);

        assert!(matches!(
            parse_l2_address(&friendly),
            Err(L2AddressError::InvalidChecksum | L2AddressError::Invalid)
        ));
    }
}
