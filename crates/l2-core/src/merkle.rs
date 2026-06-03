use crate::crypto::{hash_domain, Hash32};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MerkleProof {
    pub leaf_index: u64,
    pub siblings: Vec<Hash32>,
}

pub fn merkle_root(leaves: &[Hash32]) -> Hash32 {
    if leaves.is_empty() {
        return Hash32::ZERO;
    }

    let mut level = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity((level.len() + 1) / 2);
        for pair in level.chunks(2) {
            let left = pair[0];
            let right = *pair.get(1).unwrap_or(&pair[0]);
            next.push(hash_pair(left, right));
        }
        level = next;
    }

    level[0]
}

pub fn build_merkle_proof(leaves: &[Hash32], leaf_index: usize) -> Option<MerkleProof> {
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
            next.push(hash_pair(left, right));
        }
        index /= 2;
        level = next;
    }

    Some(MerkleProof {
        leaf_index: leaf_index as u64,
        siblings,
    })
}

pub fn verify_merkle_proof(root: Hash32, leaf: Hash32, proof: &MerkleProof) -> bool {
    let mut acc = leaf;
    let mut index = proof.leaf_index;

    for sibling in &proof.siblings {
        acc = if index % 2 == 0 {
            hash_pair(acc, *sibling)
        } else {
            hash_pair(*sibling, acc)
        };
        index /= 2;
    }

    acc == root
}

fn hash_pair(left: Hash32, right: Hash32) -> Hash32 {
    hash_domain("l2.merkle.node", &[left.as_bytes(), right.as_bytes()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::sha256_bytes;

    #[test]
    fn merkle_proof_roundtrip() {
        let leaves = ["a", "b", "c", "d"]
            .into_iter()
            .map(|v| sha256_bytes(v.as_bytes()))
            .collect::<Vec<_>>();
        let root = merkle_root(&leaves);

        for (index, leaf) in leaves.iter().enumerate() {
            let proof = build_merkle_proof(&leaves, index).expect("proof");
            assert!(verify_merkle_proof(root, *leaf, &proof));
        }
    }
}
