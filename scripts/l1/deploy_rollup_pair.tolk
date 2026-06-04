import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@acton/testing/assert"
import "@wrappers/AssetVault.gen"
import "@wrappers/RollupRoot.gen"
import "@scripts/l1/deploy_helpers"

fun main(
    sequencer: address,
    wrappedGasMinter: address,
    challengeWindowSec: uint32,
    tonAssetId: uint32,
    tonDecimals: uint8,
) {
    assertDeploymentConfig(challengeWindowSec, tonAssetId, tonDecimals);

    val deployer = scripts.wallet(deployerWalletLabel());
    val admin = deployer.address;
    val root = rollupRootFromConfig(admin, sequencer, challengeWindowSec);
    val vault = assetVaultFromConfig(
        admin,
        root.address,
        wrappedGasMinter,
        tonAssetId,
        tonDecimals,
    );

    println("deployment wallet/admin: {}", admin);
    println("rollup root planned address: {}", root.address);
    println("asset vault planned address: {}", vault.address);

    if (scripts.isContractDeployed(root.address)) {
        Assert.fail("RollupRoot planned address is already deployed; refusing replay");
    }
    if (scripts.isContractDeployed(vault.address)) {
        Assert.fail("AssetVault planned address is already deployed; refusing replay");
    }

    val rootTrace = root.deploy(admin, { value: ton("0.12") }).waitForTrace();
    if (rootTrace == null) {
        Assert.fail("RollupRoot deployment trace was not found");
    }
    val vaultTrace = vault.deploy(admin, { value: ton("0.12") }).waitForTrace();
    if (vaultTrace == null) {
        Assert.fail("AssetVault deployment trace was not found");
    }
    val linkTrace = root
        .sendSetAssetVault(admin, vault.address, { value: ton("0.05") })
        .waitForTrace();
    if (linkTrace == null) {
        Assert.fail("RollupRoot vault-link trace was not found");
    }

    assertLinkedGetters(
        root,
        vault,
        admin,
        sequencer,
        wrappedGasMinter,
        challengeWindowSec,
        tonAssetId,
        tonDecimals,
    );
    writeDeploymentJson(
        deploymentJson(
            root,
            vault,
            admin,
            sequencer,
            wrappedGasMinter,
            challengeWindowSec,
            tonAssetId,
            tonDecimals,
        ),
    );
}
