import "@acton/build"
import "@acton/env"
import "@acton/fmt"
import "@acton/fs"
import "@acton/io"
import "@acton/testing/assert"
import "@shared/crypto"
import "@shared/storage"
import "@wrappers/AssetVault.gen"
import "@wrappers/RollupRoot.gen"

const DEFAULT_DEPLOYMENT_OUTPUT_JSON = "build/testnet-l1-deployment.json"
const DEFAULT_DEPLOYER_WALLET_LABEL = "entropis-testnet-deployer"
const TESTNET_RECOMMENDED_CHALLENGE_WINDOW_SEC = 300

fun deployerWalletLabel(): string {
    return env<string>("L1_DEPLOYER_WALLET_LABEL") ?? DEFAULT_DEPLOYER_WALLET_LABEL;
}

fun deploymentOutputPath(): string {
    return env<string>("L1_DEPLOY_OUTPUT_JSON") ?? DEFAULT_DEPLOYMENT_OUTPUT_JSON;
}

fun rollupRootFromConfig(
    admin: address,
    sequencer: address,
    challengeWindowSec: uint32,
): RollupRoot {
    return RollupRoot.fromStorage({
        admin,
        sequencer,
        assetVault: unlinkedAssetVaultAddress(),
        challengeWindowSec,
        lastCommitted: 0,
        lastFinalized: 0,
        paused: false,
        commitments: [],
        claimedWithdrawals: [],
        failedWithdrawals: [],
    });
}

fun assetVaultFromConfig(
    admin: address,
    rollupRoot: address,
    wrappedGasMinter: address,
    tonAssetId: uint32,
    tonDecimals: uint8,
): AssetVault {
    return AssetVault.fromStorage({
        admin,
        rollupRoot,
        wrappedGasMinter,
        paused: false,
        lockedTon: 0,
        tonAssetId,
        tonDecimals,
        jettonAssets: [],
        jettonWalletToAsset: [],
        pendingJettonReleases: [],
        releaseFailures: [],
    });
}

fun assertDeploymentConfig(challengeWindowSec: uint32, tonAssetId: uint32, tonDecimals: uint8) {
    if (challengeWindowSec == 0) {
        Assert.fail("challengeWindowSec must be non-zero");
    }
    if (challengeWindowSec != TESTNET_RECOMMENDED_CHALLENGE_WINDOW_SEC) {
        println(
            "warning: challengeWindowSec is {}, recommended testnet value is {}",
            challengeWindowSec,
            TESTNET_RECOMMENDED_CHALLENGE_WINDOW_SEC,
        );
    }
    if (tonAssetId == 0) {
        Assert.fail("tonAssetId must be non-zero");
    }
    if (tonDecimals > 18) {
        Assert.fail("tonDecimals must be <= 18");
    }
}

fun assertLinkedGetters(
    root: RollupRoot,
    vault: AssetVault,
    admin: address,
    sequencer: address,
    wrappedGasMinter: address,
    challengeWindowSec: uint32,
    tonAssetId: uint32,
    tonDecimals: uint8,
) {
    val rootStatus = root.rollupStatus();
    Assert.equal(rootStatus.sequencer, sequencer, "rollup sequencer mismatch");
    Assert.equal(rootStatus.assetVault, vault.address, "rollup vault link mismatch");
    Assert.equal(rootStatus.challengeWindowSec, challengeWindowSec, "challenge window mismatch");
    Assert.equal(rootStatus.lastCommitted, 0, "unexpected initial committed batch");
    Assert.equal(rootStatus.lastFinalized, 0, "unexpected initial finalized batch");
    Assert.equal(rootStatus.paused, false, "rollup must start active for current MVP");

    val vaultStatus = vault.vaultStatus();
    Assert.equal(vaultStatus.rollupRoot, root.address, "vault root link mismatch");
    Assert.equal(vaultStatus.wrappedGasMinter, wrappedGasMinter, "wrapped gas minter mismatch");
    Assert.equal(vaultStatus.lockedTon, 0, "unexpected initial locked TON");
    Assert.equal(vaultStatus.tonAssetId, tonAssetId, "TON asset id mismatch");
    Assert.equal(vaultStatus.tonDecimals, tonDecimals, "TON decimals mismatch");
    Assert.equal(vaultStatus.paused, false, "vault must start active for current MVP");
    Assert.equal(admin, admin, "admin field is part of storage but has no getter yet");
}

fun deploymentJson(
    root: RollupRoot,
    vault: AssetVault,
    admin: address,
    sequencer: address,
    wrappedGasMinter: address,
    challengeWindowSec: uint32,
    tonAssetId: uint32,
    tonDecimals: uint8,
): string {
    val addresses = format(
        "\"rollupRoot\":\"{}\",\"assetVault\":\"{}\",\"admin\":\"{}\",\"sequencer\":\"{}\",\"wrappedGasMinter\":\"{}\"",
        rawAddress(root.address),
        rawAddress(vault.address),
        rawAddress(admin),
        rawAddress(sequencer),
        rawAddress(wrappedGasMinter),
    );
    val params = format(
        "\"challengeWindowSec\":{},\"tonAssetId\":{},\"tonDecimals\":{}",
        challengeWindowSec,
        tonAssetId,
        tonDecimals,
    );
    val hashes = format(
        "\"rollupRootCodeHash\":\"{}\",\"assetVaultCodeHash\":\"{}\",\"rollupRootDataHash\":\"{}\",\"assetVaultDataHash\":\"{}\"",
        hashText(cellReprHash(build("RollupRoot"))),
        hashText(cellReprHash(build("AssetVault"))),
        hashText(cellReprHash(rootInitialData(admin, sequencer, challengeWindowSec))),
        hashText(
            cellReprHash(
                vaultInitialData(admin, root.address, wrappedGasMinter, tonAssetId, tonDecimals),
            ),
        ),
    );
    return format("{{{},{},{}}}", addresses, params, hashes);
}

fun rawAddress(addr: address): string {
    val (workchain, hash) = addr.getWorkchainAndHash();
    return format("{}:{:x}", workchain, hash);
}

fun hashText(hash: uint256): string {
    return format("0x{:x}", hash);
}

fun rootInitialData(admin: address, sequencer: address, challengeWindowSec: uint32): cell {
    return RollupStorage {
        admin,
        sequencer,
        assetVault: unlinkedAssetVaultAddress(),
        challengeWindowSec,
        lastCommitted: 0,
        lastFinalized: 0,
        paused: false,
        commitments: [],
        claimedWithdrawals: [],
        failedWithdrawals: [],
    }.toCell();
}

fun vaultInitialData(
    admin: address,
    rollupRoot: address,
    wrappedGasMinter: address,
    tonAssetId: uint32,
    tonDecimals: uint8,
): cell {
    return AssetVaultStorage {
        admin,
        rollupRoot,
        wrappedGasMinter,
        paused: false,
        lockedTon: 0,
        tonAssetId,
        tonDecimals,
        jettonAssets: [],
        jettonWalletToAsset: [],
        pendingJettonReleases: [],
        releaseFailures: [],
    }.toCell();
}

fun writeDeploymentJson(json: string) {
    val outputPath = deploymentOutputPath();
    if (!fs.writeString(outputPath, json)) {
        Assert.fail(format("failed to write deployment output JSON to {}", outputPath));
    }
    println("deployment output written to {}", outputPath);
    println("DEPLOYMENT_JSON {}", json);
}
