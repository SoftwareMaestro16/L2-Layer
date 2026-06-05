import "@acton/io"
import "@wrappers/AssetVault.gen"
import "@wrappers/RollupRoot.gen"
import "@scripts/l1/deploy_helpers"

fun main(
    rollupRootAddress: address,
    assetVaultAddress: address,
    admin: address,
    sequencer: address,
    wrappedGasMinter: address,
    challengeWindowSec: uint32,
    tonAssetId: uint32,
    tonDecimals: uint8,
) {
    assertDeploymentConfig(challengeWindowSec, tonAssetId, tonDecimals);

    val root = RollupRoot.fromAddress(rollupRootAddress);
    val vault = AssetVault.fromAddress(assetVaultAddress);

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

    println("exported verified deployment for RollupRoot {}", root.address);
}
