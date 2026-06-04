import "@acton/io"
import "@wrappers/AssetVault.gen"
import "@wrappers/RollupRoot.gen"
import "@scripts/l1/deploy_helpers"

fun main(
    rollupRoot: address,
    assetVault: address,
    admin: address,
    sequencer: address,
    wrappedGasMinter: address,
    challengeWindowSec: uint32,
    tonAssetId: uint32,
    tonDecimals: uint8,
) {
    val root = RollupRoot.fromAddress(rollupRoot);
    val vault = AssetVault.fromAddress(assetVault);

    assertDeploymentConfig(challengeWindowSec, tonAssetId, tonDecimals);
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

    println("verified RollupRoot {} and AssetVault {}", rollupRoot, assetVault);
}
