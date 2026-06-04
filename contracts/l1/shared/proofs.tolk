import "@shared/crypto"

const WITHDRAWAL_PROOF_CHUNK_MAX = 3

struct WithdrawalMerkleProof {
    leafIndex: uint64
    siblingsCount: uint16
    first: Cell<WithdrawalProofChunk>?
}

struct WithdrawalProofChunk {
    count: uint8
    sibling0: uint256
    sibling1: uint256
    sibling2: uint256
    next: Cell<WithdrawalProofChunk>?
}

/// Hashes a withdrawal tree node using the same compact TON-cell layout used by Rust proof generation.
fun hashWithdrawalNode(left: uint256, right: uint256): uint256 {
    return cellReprHash(
        beginCell().storeUint(left, 256).storeUint(right, 256).endCell(),
    );
}

fun hashWithdrawalSibling(acc: uint256, sibling: uint256, index: uint64): uint256 {
    if (index % 2 == 0) {
        return hashWithdrawalNode(acc, sibling);
    }
    return hashWithdrawalNode(sibling, acc);
}

/// Verifies inclusion of a ReleaseAuthorized leaf cell in the committed withdrawal root.
fun verifyWithdrawalProof(root: uint256, withdrawalLeaf: cell, merkleProof: cell): bool {
    val proof = WithdrawalMerkleProof.fromCell(merkleProof);
    var acc = cellReprHash(withdrawalLeaf);
    var index = proof.leafIndex;
    var remaining = proof.siblingsCount;
    var next = proof.first;

    if (remaining == 0) {
        return next == null && acc == root;
    }

    while (remaining > 0) {
        if (next == null) {
            return false;
        }

        val chunk = next!.load();
        if (
            chunk.count == 0 || chunk.count > WITHDRAWAL_PROOF_CHUNK_MAX || chunk.count > remaining
        ) {
            return false;
        }

        var applied = 0;
        if (chunk.count >= 1) {
            acc = hashWithdrawalSibling(acc, chunk.sibling0, index);
            index = index / 2;
            applied = applied + 1;
        }
        if (chunk.count >= 2) {
            acc = hashWithdrawalSibling(acc, chunk.sibling1, index);
            index = index / 2;
            applied = applied + 1;
        }
        if (chunk.count >= 3) {
            acc = hashWithdrawalSibling(acc, chunk.sibling2, index);
            index = index / 2;
            applied = applied + 1;
        }

        remaining = remaining - applied;
        next = chunk.next;
    }

    return next == null && acc == root;
}
