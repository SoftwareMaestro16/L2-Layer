import "@acton/env"
import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@acton/testing/assert"
import "@wrappers/RollupRoot.gen"

const DEFAULT_CLAIM_WALLET_LABEL = "entropis-testnet-deployer"
const DEFAULT_CLAIM_MSG_VALUE_NANOTON = 150000000

fun claimWalletLabel(): string {
    return env<string>("L1_CLAIM_WALLET_LABEL") ?? DEFAULT_CLAIM_WALLET_LABEL;
}

fun claimMsgValue(): coins {
    return env<coins>("L1_CLAIM_MSG_VALUE_NANOTON") ?? DEFAULT_CLAIM_MSG_VALUE_NANOTON;
}

fun main(
    rollupRoot: address,
    batchNo: uint64,
    withdrawalId: uint256,
    withdrawalLeaf: cell,
    merkleProof: cell,
) {
    if (batchNo == 0) {
        Assert.fail("batchNo must be one-based");
    }
    if (withdrawalId == 0) {
        Assert.fail("withdrawalId must be non-zero");
    }

    val claimant = scripts.wallet(claimWalletLabel());
    val root = RollupRoot.fromAddress(rollupRoot);

    println("claim wallet: {}", claimant.address);
    println("rollup root: {}", root.address);
    println("claiming batch: {}", batchNo);
    println("withdrawal id: 0x{:x}", withdrawalId);

    val trace = root
        .sendClaimWithdrawal(
            claimant.address,
            batchNo,
            withdrawalId,
            withdrawalLeaf,
            merkleProof,
            { value: claimMsgValue() },
        )
        .waitForTrace();
    if (trace == null) {
        Assert.fail("ClaimWithdrawal trace was not found");
    }

    println("ClaimWithdrawal sent for withdrawal 0x{:x}", withdrawalId);
}
