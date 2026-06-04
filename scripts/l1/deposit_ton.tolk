import "@acton/env"
import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@acton/testing/assert"
import "@wrappers/AssetVault.gen"

const DEFAULT_DEPOSIT_WALLET_LABEL = "entropis-testnet-deployer"
const DEFAULT_DEPOSIT_FEE_BUFFER_NANOTON = 20000000

fun depositWalletLabel(): string {
    return env<string>("L1_DEPOSIT_WALLET_LABEL") ?? DEFAULT_DEPOSIT_WALLET_LABEL;
}

fun depositFeeBuffer(): coins {
    return env<coins>("L1_DEPOSIT_FEE_BUFFER_NANOTON") ?? DEFAULT_DEPOSIT_FEE_BUFFER_NANOTON;
}

fun main(assetVault: address, amount: coins, l2Recipient: uint256, queryId: uint64) {
    if (amount == 0) {
        Assert.fail("amount must be non-zero");
    }
    if (l2Recipient == 0) {
        Assert.fail("l2Recipient must be non-zero");
    }

    val depositor = scripts.wallet(depositWalletLabel());
    val vault = AssetVault.fromAddress(assetVault);
    val attachedValue = amount + depositFeeBuffer();

    println("deposit wallet: {}", depositor.address);
    println("asset vault: {}", vault.address);
    println("credited TON amount: {}", amount);
    println("attached message value: {}", attachedValue);
    println("l2 recipient: 0x{:x}", l2Recipient);
    println("query id: {}", queryId);

    val trace = vault
        .sendDepositTon(depositor.address, queryId, amount, l2Recipient, { value: attachedValue })
        .waitForTrace();
    if (trace == null) {
        Assert.fail("DepositTon trace was not found");
    }

    println("DepositTon sent to {}", vault.address);
}
