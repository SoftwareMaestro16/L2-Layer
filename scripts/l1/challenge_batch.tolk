import "@acton/env"
import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@acton/testing/assert"
import "@shared/messages"
import "@wrappers/RollupRoot.gen"

const DEFAULT_CHALLENGE_WALLET_LABEL = "entropis-testnet-deployer"
const DEFAULT_CHALLENGE_FEE_BUFFER_NANOTON = 20000000

fun challengeWalletLabel(): string {
    return env<string>("L1_CHALLENGE_WALLET_LABEL") ?? DEFAULT_CHALLENGE_WALLET_LABEL;
}

fun challengeFeeBuffer(): coins {
    return env<coins>("L1_CHALLENGE_FEE_BUFFER_NANOTON") ?? DEFAULT_CHALLENGE_FEE_BUFFER_NANOTON;
}

fun main(rollupRoot: address, batchNo: uint64, reason: uint8, evidenceHash: uint256) {
    if (batchNo == 0) {
        Assert.fail("batchNo must be one-based");
    }
    if (reason == 0) {
        Assert.fail("challenge reason must be non-zero");
    }
    if (evidenceHash == 0) {
        Assert.fail("evidenceHash must be non-zero");
    }

    val challenger = scripts.wallet(challengeWalletLabel());
    val root = RollupRoot.fromAddress(rollupRoot);
    val status = root.rollupStatus();
    if (status.lastCommitted < batchNo) {
        Assert.fail("batch is not committed yet");
    }
    if (status.lastFinalized >= batchNo) {
        Assert.fail("batch is already finalized");
    }

    val existing = root.batchChallenge(batchNo);
    if (existing.exists) {
        Assert.fail("batch challenge already exists");
    }

    val attachedValue = MIN_CHALLENGE_BOND_NANOTON + challengeFeeBuffer();

    println("challenge wallet: {}", challenger.address);
    println("rollup root: {}", root.address);
    println("challenging batch: {}", batchNo);
    println("reason: {}", reason);
    println("evidence hash: 0x{:x}", evidenceHash);
    println("challenger bond: {}", MIN_CHALLENGE_BOND_NANOTON);
    println("attached message value: {}", attachedValue);

    val trace = root
        .sendChallengeBatch(
            challenger.address,
            batchNo,
            reason,
            evidenceHash,
            MIN_CHALLENGE_BOND_NANOTON,
            { value: attachedValue },
        )
        .waitForTrace();
    if (trace == null) {
        Assert.fail("ChallengeBatch trace was not found");
    }

    val challenge = root.batchChallenge(batchNo);
    if (!challenge.exists) {
        Assert.fail("challenge was not stored");
    }

    println("ChallengeBatch accepted for batch {}", batchNo);
}
