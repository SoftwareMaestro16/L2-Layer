import "@acton/env"
import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@acton/testing/assert"
import "@shared/messages"
import "@wrappers/RollupRoot.gen"

const DEFAULT_RESOLVE_WALLET_LABEL = "entropis-testnet-deployer"
const DEFAULT_RESOLVE_MSG_VALUE_NANOTON = 100000000

fun resolveWalletLabel(): string {
    return env<string>("L1_RESOLVE_WALLET_LABEL") ?? DEFAULT_RESOLVE_WALLET_LABEL;
}

fun resolveMsgValue(): coins {
    return env<coins>("L1_RESOLVE_MSG_VALUE_NANOTON") ?? DEFAULT_RESOLVE_MSG_VALUE_NANOTON;
}

fun main(rollupRoot: address, batchNo: uint64, uphold: bool, slashAmount: coins) {
    if (batchNo == 0) {
        Assert.fail("batchNo must be one-based");
    }
    if (uphold && slashAmount == 0) {
        Assert.fail("upheld challenge must slash a non-zero amount");
    }
    if (!uphold && slashAmount != 0) {
        Assert.fail("rejected challenge must not slash bond");
    }

    val admin = scripts.wallet(resolveWalletLabel());
    val root = RollupRoot.fromAddress(rollupRoot);
    val challenge = root.batchChallenge(batchNo);
    if (!challenge.exists) {
        Assert.fail("challenge does not exist");
    }
    if (challenge.challenge.load().status != CHALLENGE_STATUS_OPEN) {
        Assert.fail("challenge is not open");
    }

    val bond = root.bondStatus();
    if (uphold && bond.sequencerBond < slashAmount) {
        Assert.fail("slashAmount exceeds sequencer bond");
    }

    println("resolve wallet/admin: {}", admin.address);
    println("rollup root: {}", root.address);
    println("resolving batch challenge: {}", batchNo);
    println("uphold: {}", uphold);
    println("slash amount: {}", slashAmount);

    val trace = root
        .sendResolveChallenge(
            admin.address,
            batchNo,
            uphold,
            slashAmount,
            { value: resolveMsgValue() },
        )
        .waitForTrace();
    if (trace == null) {
        Assert.fail("ResolveChallenge trace was not found");
    }

    val resolved = root.batchChallenge(batchNo).challenge.load();
    if (resolved.status == CHALLENGE_STATUS_OPEN) {
        Assert.fail("challenge was not resolved");
    }

    println("ResolveChallenge accepted for batch {}", batchNo);
}
