import "@acton/env"
import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@acton/testing/assert"
import "@wrappers/RollupRoot.gen"

const DEFAULT_STAKE_WALLET_LABEL = "entropis-testnet-sequencer"
const DEFAULT_STAKE_FEE_BUFFER_NANOTON = 20000000

fun stakeWalletLabel(): string {
    return env<string>("L1_STAKE_WALLET_LABEL") ?? DEFAULT_STAKE_WALLET_LABEL;
}

fun stakeFeeBuffer(): coins {
    return env<coins>("L1_STAKE_FEE_BUFFER_NANOTON") ?? DEFAULT_STAKE_FEE_BUFFER_NANOTON;
}

fun main(rollupRoot: address, amount: coins) {
    if (amount == 0) {
        Assert.fail("bond amount must be non-zero");
    }

    val sequencer = scripts.wallet(stakeWalletLabel());
    val root = RollupRoot.fromAddress(rollupRoot);
    val status = root.rollupStatus();
    if (status.sequencer != sequencer.address) {
        Assert.fail("stake wallet does not match RollupRoot.sequencer");
    }

    val before = root.bondStatus();
    val attachedValue = amount + stakeFeeBuffer();

    println("stake wallet: {}", sequencer.address);
    println("rollup root: {}", root.address);
    println("staking sequencer bond: {}", amount);
    println("attached message value: {}", attachedValue);

    val trace = root
        .sendStakeSequencerBond(sequencer.address, amount, { value: attachedValue })
        .waitForTrace();
    if (trace == null) {
        Assert.fail("StakeSequencerBond trace was not found");
    }

    val after = root.bondStatus();
    if (after.sequencerBond < before.sequencerBond + amount) {
        Assert.fail("sequencer bond did not increase");
    }

    println("StakeSequencerBond accepted; sequencerBond={}", after.sequencerBond);
}
