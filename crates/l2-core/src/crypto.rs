use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::fmt;

#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Hash32(pub [u8; 32]);

impl Hash32 {
    pub const ZERO: Self = Self([0; 32]);

    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn from_hex(value: &str) -> Result<Self, hex::FromHexError> {
        let cleaned = value.strip_prefix("0x").unwrap_or(value);
        let bytes = hex::decode(cleaned)?;
        let mut out = [0u8; 32];
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        out.copy_from_slice(&bytes);
        Ok(Self(out))
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Debug for Hash32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", self.to_hex())
    }
}

impl fmt::Display for Hash32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl Serialize for Hash32 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Hash32 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Hash32::from_hex(&value).map_err(serde::de::Error::custom)
    }
}

pub fn sha256_bytes(bytes: &[u8]) -> Hash32 {
    let digest = Sha256::digest(bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    Hash32(out)
}

pub fn hash_domain(domain: &str, parts: &[&[u8]]) -> Hash32 {
    let mut hasher = Sha256::new();
    hasher.update((domain.len() as u64).to_be_bytes());
    hasher.update(domain.as_bytes());
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    Hash32(out)
}

pub fn derive_account_id(public_key: &[u8; 32]) -> Hash32 {
    hash_domain("l2.account.ed25519", &[public_key])
}

pub fn verify_signature(public_key_hex: &str, signature_hex: &str, payload: &[u8]) -> bool {
    let public_key = match decode_fixed::<32>(public_key_hex) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let signature = match decode_fixed::<64>(signature_hex) {
        Ok(value) => value,
        Err(_) => return false,
    };

    let verifying_key = match VerifyingKey::from_bytes(&public_key) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let signature = Signature::from_bytes(&signature);
    verifying_key.verify(payload, &signature).is_ok()
}

pub fn decode_fixed<const N: usize>(value: &str) -> Result<[u8; N], hex::FromHexError> {
    let cleaned = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(cleaned)?;
    if bytes.len() != N {
        return Err(hex::FromHexError::InvalidStringLength);
    }

    let mut out = [0u8; N];
    out.copy_from_slice(&bytes);
    Ok(out)
}
