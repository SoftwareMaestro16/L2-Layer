import "@acton/env"
import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@acton/testing/assert"
import "@shared/messages"
import "@wrappers/RollupRoot.gen"

const DEFAULT_SEQUENCER_WALLET_LABEL = "entropis-testnet-sequencer"
const DEFAULT_COMMIT_MSG_VALUE_NANOTON = 100000000

fun sequencerWalletLabel(): string {
    return env<string>("L1_SEQUENCER_WALLET_LABEL") ?? DEFAULT_SEQUENCER_WALLET_LABEL;
}

fun commitMsgValue(): coins {
    return env<coins>("L1_COMMIT_MSG_VALUE_NANOTON") ?? DEFAULT_COMMIT_MSG_VALUE_NANOTON;
}

fun main(
    rollupRoot: address,
    batchNo: uint64,
    prevStateRoot: uint256,
    stateRoot: uint256,
    txRoot: uint256,
    receiptRoot: uint256,
    withdrawalRoot: uint256,
    dataHash: uint256,
) {
    if (batchNo == 0) {
        Assert.fail("batchNo must be one-based");
    }

    val sequencer = scripts.wallet(sequencerWalletLabel());
    val root = RollupRoot.fromAddress(rollupRoot);
    val statusBefore = root.rollupStatus();
    if (statusBefore.sequencer != sequencer.address) {
        Assert.fail("sequencer wallet does not match RollupRoot.sequencer");
    }
    if (statusBefore.lastCommitted + 1 != batchNo) {
        Assert.fail("batchNo is not the next RollupRoot batch");
    }

    val rootsA = BatchRootsA { prevStateRoot, stateRoot, txRoot };
    val rootsB = BatchRootsB { receiptRoot, withdrawalRoot, dataHash };

    println("sequencer wallet: {}", sequencer.address);
    println("rollup root: {}", root.address);
    println("committing batch: {}", batchNo);
    println("prev_state_root: 0x{:x}", prevStateRoot);
    println("state_root: 0x{:x}", stateRoot);
    println("tx_root: 0x{:x}", txRoot);
    println("receipt_root: 0x{:x}", receiptRoot);
    println("withdrawal_root: 0x{:x}", withdrawalRoot);
    println("data_hash: 0x{:x}", dataHash);

    val trace = root
        .sendCommitBatch(sequencer.address, batchNo, rootsA, rootsB, { value: commitMsgValue() })
        .waitForTrace();
    if (trace == null) {
        Assert.fail("CommitBatch trace was not found");
    }

    val statusAfter = root.rollupStatus();
    if (statusAfter.lastCommitted < batchNo) {
        Assert.fail("RollupRoot lastCommitted did not advance");
    }

    println("CommitBatch accepted for batch {}", batchNo);
}
