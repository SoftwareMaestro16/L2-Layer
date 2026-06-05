import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@shared/errors"
import "@shared/messages"
import "@shared/storage"
import "@wrappers/AssetVault.gen"
import "@wrappers/RollupRoot.gen"

get fun `test deployment links root and vault after deterministic root deploy`() {
    val (root, vault, admin, sequencer) = deployLinkablePair();

    val link = root.sendSetAssetVault(admin.address, vault.address, { value: ton("0.05") });
    expect(link).toHaveSuccessfulTx<SetAssetVault>({
        from: admin.address,
        to: root.address,
    });

    val rootStatus = root.rollupStatus();
    expect(rootStatus.sequencer).toEqual(sequencer.address);
    expect(rootStatus.assetVault).toEqual(vault.address);
    expect(rootStatus.challengeWindowSec).toEqual(300);
    expect(rootStatus.paused).toEqual(false);

    val vaultStatus = vault.vaultStatus();
    expect(vaultStatus.rollupRoot).toEqual(root.address);
    expect(vaultStatus.paused).toEqual(false);
}

get fun `test deployment rejects unauthorized root vault link`() {
    val (root, vault, _, _) = deployLinkablePair();
    val attacker = testing.treasury("deployment-link-attacker");

    val link = root.sendSetAssetVault(attacker.address, vault.address, { value: ton("0.05") });
    expect(link).toHaveFailedTx({
        from: attacker.address,
        to: root.address,
        exitCode: Errors.Unauthorized,
    });
    expect(root.rollupStatus().assetVault).toEqual(unlinkedAssetVaultAddress());
}

get fun `test deployment rejects empty and replayed root vault links`() {
    val (root, vault, admin, _) = deployLinkablePair();

    val emptyLink = root.sendSetAssetVault(
        admin.address,
        unlinkedAssetVaultAddress(),
        { value: ton("0.05") },
    );
    expect(emptyLink).toHaveFailedTx({
        from: admin.address,
        to: root.address,
        exitCode: Errors.BadAssetConfig,
    });

    root.sendSetAssetVault(admin.address, vault.address, { value: ton("0.05") });
    val replay = root.sendSetAssetVault(admin.address, vault.address, { value: ton("0.05") });
    expect(replay).toHaveFailedTx({
        from: admin.address,
        to: root.address,
        exitCode: Errors.AlreadyConfigured,
    });
}

get fun `test deployment rejects batch commit before root vault link`() {
    val (root, vault, admin, sequencer) = deployLinkablePair();

    val unlinkedCommit = root.sendCommitBatch(
        sequencer.address,
        1,
        BatchRootsA {
            prevStateRoot: 0,
            stateRoot: 11,
            txRoot: 12,
        },
        BatchRootsB {
            receiptRoot: 13,
            withdrawalRoot: 14,
            dataHash: 15,
        },
        { value: ton("0.1") },
    );
    expect(unlinkedCommit).toHaveFailedTx({
        from: sequencer.address,
        to: root.address,
        exitCode: Errors.BadAssetConfig,
    });

    val link = root.sendSetAssetVault(admin.address, vault.address, { value: ton("0.05") });
    expect(link).toHaveSuccessfulTx<SetAssetVault>({
        from: admin.address,
        to: root.address,
    });
}

fun deployLinkablePair(): (RollupRoot, AssetVault, Treasury, Treasury) {
    val admin = testing.treasury("deployment-link-admin");
    val sequencer = testing.treasury("deployment-link-sequencer");
    val wrappedGasMinter = testing.treasury("deployment-link-wrapped-gas-minter");

    val root = RollupRoot.fromStorage({
        admin: admin.address,
        sequencer: sequencer.address,
        assetVault: unlinkedAssetVaultAddress(),
        challengeWindowSec: 300,
        lastCommitted: 0,
        lastFinalized: 0,
        paused: false,
        security: emptyRollupSecurityState().toCell(),
        commitments: [],
        claimedWithdrawals: [],
        failedWithdrawals: [],
    });
    root.deploy(admin.address, { value: ton("0.1") });

    val vault = AssetVault.fromStorage({
        admin: admin.address,
        rollupRoot: root.address,
        wrappedGasMinter: wrappedGasMinter.address,
        paused: false,
        lockedTon: 0,
        tonAssetId: 1,
        tonDecimals: 9,
        jettonAssets: [],
        jettonWalletToAsset: [],
        pendingJettonReleases: [],
        releaseFailures: [],
    });
    vault.deploy(admin.address, { value: ton("0.1") });

    return (root, vault, admin, sequencer);
}
