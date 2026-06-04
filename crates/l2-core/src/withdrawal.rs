use crate::crypto::Hash32;
use crate::merkle::MerkleProof;
use crate::types::WithdrawalLeaf;
use num_bigint::BigUint;
use thiserror::Error;
use tonlib_core::cell::{CellBuilder, TonCellError};
use tonlib_core::types::{TonAddress, TonAddressParseError};

pub const RELEASE_AUTHORIZED_OPCODE: u32 = 0x4c325206;
pub const MAX_TON_COINS: u128 = (1u128 << 120) - 1;

#[derive(Debug, Error)]
pub enum WithdrawalProofError {
    #[error("invalid l1 recipient address")]
    InvalidRecipient(#[from] TonAddressParseError),
    #[error("withdrawal amount exceeds TON coins encoding limit")]
    AmountTooLarge,
    #[error("ton cell build failed")]
    CellBuild(#[from] TonCellError),
}

impl WithdrawalProofError {
    pub fn rejection_reason(&self) -> &'static str {
        match self {
            Self::InvalidRecipient(_) => "bad_l1_recipient",
            Self::AmountTooLarge => "withdrawal_amount_too_large",
            Self::CellBuild(_) => "withdrawal_cell_build_failed",
        }
    }
}

pub fn validate_release_parts(
    amount: u128,
    l1_recipient: &str,
) -> Result<(), WithdrawalProofError> {
    validate_amount(amount)?;
    parse_ton_address(l1_recipient)?;
    Ok(())
}

pub fn release_leaf_hash(leaf: &WithdrawalLeaf) -> Result<Hash32, WithdrawalProofError> {
    release_authorized_cell_hash(
        leaf.withdrawal_id,
        leaf.asset_id,
        leaf.amount,
        &leaf.l1_recipient,
    )
}

pub fn withdrawal_merkle_root(leaves: &[WithdrawalLeaf]) -> Result<Hash32, WithdrawalProofError> {
    let hashes = withdrawal_leaf_hashes(leaves)?;
    Ok(merkle_root_from_hashes(&hashes))
}

pub fn build_withdrawal_merkle_proof(
    leaves: &[WithdrawalLeaf],
    leaf_index: usize,
) -> Result<Option<MerkleProof>, WithdrawalProofError> {
    let hashes = withdrawal_leaf_hashes(leaves)?;
    Ok(build_merkle_proof_from_hashes(&hashes, leaf_index))
}

pub fn verify_withdrawal_merkle_proof(
    root: Hash32,
    leaf: &WithdrawalLeaf,
    proof: &MerkleProof,
) -> Result<bool, WithdrawalProofError> {
    let leaf_hash = release_leaf_hash(leaf)?;
    Ok(verify_hash_proof(root, leaf_hash, proof))
}

pub fn withdrawal_leaf_hashes(
    leaves: &[WithdrawalLeaf],
) -> Result<Vec<Hash32>, WithdrawalProofError> {
    leaves.iter().map(release_leaf_hash).collect()
}

fn release_authorized_cell_hash(
    withdrawal_id: Hash32,
    asset_id: u32,
    amount: u128,
    l1_recipient: &str,
) -> Result<Hash32, WithdrawalProofError> {
    validate_amount(amount)?;
    let recipient = parse_ton_address(l1_recipient)?;
    let amount = BigUint::from(amount);

    let cell = CellBuilder::new()
        .store_u32(32, RELEASE_AUTHORIZED_OPCODE)?
        .store_bits(256, withdrawal_id.as_bytes())?
        .store_u32(32, asset_id)?
        .store_address(&recipient)?
        .store_coins(&amount)?
        .build()?;
    Ok(hash_from_slice(cell.cell_hash().as_slice()))
}

fn merkle_root_from_hashes(leaves: &[Hash32]) -> Hash32 {
    if leaves.is_empty() {
        return Hash32::ZERO;
    }

    let mut level = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity((level.len() + 1) / 2);
        for pair in level.chunks(2) {
            let left = pair[0];
            let right = *pair.get(1).unwrap_or(&pair[0]);
            next.push(hash_withdrawal_node(left, right));
        }
        level = next;
    }
    level[0]
}

fn build_merkle_proof_from_hashes(leaves: &[Hash32], leaf_index: usize) -> Option<MerkleProof> {
    if leaf_index >= leaves.len() {
        return None;
    }

    let mut index = leaf_index;
    let mut level = leaves.to_vec();
    let mut siblings = Vec::new();

    while level.len() > 1 {
        let sibling_index = if index % 2 == 0 { index + 1 } else { index - 1 };
        siblings.push(*level.get(sibling_index).unwrap_or(&level[index]));

        let mut next = Vec::with_capacity((level.len() + 1) / 2);
        for pair in level.chunks(2) {
            let left = pair[0];
            let right = *pair.get(1).unwrap_or(&pair[0]);
            next.push(hash_withdrawal_node(left, right));
        }
        index /= 2;
        level = next;
    }

    Some(MerkleProof {
        leaf_index: leaf_index as u64,
        siblings,
    })
}

fn verify_hash_proof(root: Hash32, leaf: Hash32, proof: &MerkleProof) -> bool {
    let mut acc = leaf;
    let mut index = proof.leaf_index;

    for sibling in &proof.siblings {
        acc = if index % 2 == 0 {
            hash_withdrawal_node(acc, *sibling)
        } else {
            hash_withdrawal_node(*sibling, acc)
        };
        index /= 2;
    }

    acc == root
}

pub fn hash_withdrawal_node(left: Hash32, right: Hash32) -> Hash32 {
    let cell = CellBuilder::new()
        .store_bits(256, left.as_bytes())
        .expect("left hash is always 256 bits")
        .store_bits(256, right.as_bytes())
        .expect("right hash is always 256 bits")
        .build()
        .expect("two uint256 values fit in one TON cell");
    hash_from_slice(cell.cell_hash().as_slice())
}

fn validate_amount(amount: u128) -> Result<(), WithdrawalProofError> {
    if amount > MAX_TON_COINS {
        return Err(WithdrawalProofError::AmountTooLarge);
    }
    Ok(())
}

fn parse_ton_address(value: &str) -> Result<TonAddress, TonAddressParseError> {
    TonAddress::from_base64_url(value)
        .or_else(|_| TonAddress::from_base64_std(value))
        .or_else(|_| TonAddress::from_hex_str(value))
}

fn hash_from_slice(bytes: &[u8]) -> Hash32 {
    let mut out = [0u8; 32];
    out.copy_from_slice(bytes);
    Hash32::new(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::sha256_bytes;

    const RECIPIENT: &str = "EQDk2VTvn04SUKJrW7rXahzdF8_Qi6utb0wj43InCu9vdjrR";

    fn withdrawal(seed: u8, amount: u128) -> WithdrawalLeaf {
        WithdrawalLeaf::new(
            sha256_bytes(&[seed]),
            1,
            amount,
            sha256_bytes(&[seed, 1]),
            RECIPIENT.to_owned(),
        )
    }

    #[test]
    fn release_authorized_leaf_hash_is_stable() {
        let leaf = withdrawal(1, 300_000_000);
        let hash = release_leaf_hash(&leaf).expect("valid release leaf");

        assert_eq!(
            hex::encode(hash.as_bytes()),
            "206ba4b2d3b80535c59d77a2ef1f5342ad31c8b552562a4c38af310bfd5557dc"
        );
    }

    #[test]
    fn withdrawal_merkle_proof_roundtrips() {
        let leaves = vec![withdrawal(1, 100), withdrawal(2, 200), withdrawal(3, 300)];
        let root = withdrawal_merkle_root(&leaves).expect("root");

        for (index, leaf) in leaves.iter().enumerate() {
            let proof = build_withdrawal_merkle_proof(&leaves, index)
                .expect("proof build")
                .expect("proof");
            assert!(verify_withdrawal_merkle_proof(root, leaf, &proof).expect("verify"));
        }
    }

    #[test]
    fn withdrawal_proof_vector_is_stable() {
        let leaves = vec![withdrawal(1, 100), withdrawal(2, 200), withdrawal(3, 300)];
        let root = withdrawal_merkle_root(&leaves).expect("root");
        let proof = build_withdrawal_merkle_proof(&leaves, 1)
            .expect("proof build")
            .expect("proof");

        assert_eq!(
            hex::encode(root.as_bytes()),
            "d5e8e681563ae874899124c32b8bb43072a4d95e0b05b2bf9ddda9ce9d5b62cf"
        );
        assert_eq!(
            hex::encode(leaves[1].withdrawal_id.as_bytes()),
            "bd99c87fa8471211c1fab534ab56b4b5f4d662ecc037f305951eef358d17fad1"
        );
        assert_eq!(proof.leaf_index, 1);
        assert_eq!(
            proof
                .siblings
                .iter()
                .map(|sibling| hex::encode(sibling.as_bytes()))
                .collect::<Vec<_>>(),
            vec![
                "c0f52e7163104fbc3d88592927dd407bfb52f59366bb9ab2eaa354984bf5341e",
                "f93417c921216f9c718722963393bf14ec8183afc14559ddf07b302cabb297ac",
            ]
        );
        assert!(verify_withdrawal_merkle_proof(root, &leaves[1], &proof).expect("verify"));
    }

    #[test]
    fn corrupted_sibling_order_fails() {
        let leaves = vec![withdrawal(1, 100), withdrawal(2, 200)];
        let root = withdrawal_merkle_root(&leaves).expect("root");
        let mut proof = build_withdrawal_merkle_proof(&leaves, 0)
            .expect("proof build")
            .expect("proof");
        proof.leaf_index = 1;

        assert!(!verify_withdrawal_merkle_proof(root, &leaves[0], &proof).expect("verify"));
    }

    #[test]
    fn invalid_recipient_is_rejected() {
        let mut leaf = withdrawal(1, 100);
        leaf.l1_recipient = "not-a-ton-address".to_owned();

        assert!(matches!(
            release_leaf_hash(&leaf),
            Err(WithdrawalProofError::InvalidRecipient(_))
        ));
    }

    #[test]
    fn amount_above_ton_coins_limit_is_rejected() {
        let leaf = withdrawal(1, MAX_TON_COINS + 1);

        assert!(matches!(
            release_leaf_hash(&leaf),
            Err(WithdrawalProofError::AmountTooLarge)
        ));
    }
}
