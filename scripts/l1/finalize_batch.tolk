import "@acton/env"
import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@acton/testing/assert"
import "@wrappers/RollupRoot.gen"

const DEFAULT_FINALIZE_WALLET_LABEL = "entropis-testnet-sequencer"
const DEFAULT_FINALIZE_MSG_VALUE_NANOTON = 100000000

fun finalizeWalletLabel(): string {
    return env<string>("L1_FINALIZE_WALLET_LABEL") ?? DEFAULT_FINALIZE_WALLET_LABEL;
}

fun finalizeMsgValue(): coins {
    return env<coins>("L1_FINALIZE_MSG_VALUE_NANOTON") ?? DEFAULT_FINALIZE_MSG_VALUE_NANOTON;
}

fun main(rollupRoot: address, batchNo: uint64) {
    if (batchNo == 0) {
        Assert.fail("batchNo must be one-based");
    }

    val wallet = scripts.wallet(finalizeWalletLabel());
    val root = RollupRoot.fromAddress(rollupRoot);
    val statusBefore = root.rollupStatus();
    if (statusBefore.lastCommitted < batchNo) {
        Assert.fail("batch is not committed yet");
    }
    if (statusBefore.lastFinalized >= batchNo) {
        Assert.fail("batch is already finalized");
    }

    println("finalize wallet: {}", wallet.address);
    println("rollup root: {}", root.address);
    println("finalizing batch: {}", batchNo);

    val trace = root
        .sendFinalizeBatch(wallet.address, batchNo, { value: finalizeMsgValue() })
        .waitForTrace();
    if (trace == null) {
        Assert.fail("FinalizeBatch trace was not found");
    }

    val statusAfter = root.rollupStatus();
    if (statusAfter.lastFinalized < batchNo) {
        Assert.fail("RollupRoot lastFinalized did not advance");
    }

    println("FinalizeBatch accepted for batch {}", batchNo);
}
