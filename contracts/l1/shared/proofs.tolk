import "crypto"

// TODO: replace this placeholder with a gas-profiled Tolk verifier for the exact
// withdrawal tree encoding used by the Rust sequencer. The contract interface
// already forces callers to submit the leaf and proof as cells, so the verifier
// can be upgraded without changing public messages.
fun verifyWithdrawalProof(root: uint256, withdrawalLeaf: cell, merkleProof: cell): bool {
    return (root != 0) & (cellReprHash(withdrawalLeaf) != 0) & (cellReprHash(merkleProof) != 0);
}
