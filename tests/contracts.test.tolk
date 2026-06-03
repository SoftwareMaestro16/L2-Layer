import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"

import "@shared/errors"
import "@shared/messages"
import "@shared/storage"
import "@wrappers/AssetVault.gen"
import "@wrappers/RollupRoot.gen"

get fun `test rollup root deploy exposes status`() {
    val (root, sequencer, vaultAddress) = setupRoot(30);

    val status = root.rollupStatus();
    expect(status.sequencer).toEqual(sequencer.address);
    expect(status.assetVault).toEqual(vaultAddress);
    expect(status.challengeWindowSec).toEqual(30);
    expect(status.lastCommitted).toEqual(0);
    expect(status.lastFinalized).toEqual(0);
    expect(status.paused).toEqual(false);
}

get fun `test sequencer commits and finalizes batch after challenge window`() {
    val (root, sequencer, _) = setupRoot(10);
    testing.setNow(100);

    val rootsA = BatchRootsA {
        prevStateRoot: 0,
        stateRoot: 11,
        txRoot: 12,
    };
    val rootsB = BatchRootsB {
        receiptRoot: 13,
        withdrawalRoot: 14,
        dataHash: 15,
    };

    val commit = root.sendCommitBatch(sequencer.address, 1, rootsA, rootsB, { value: ton("0.1") });
    expect(commit).toHaveSuccessfulTx<CommitBatch>({
        from: sequencer.address,
        to: root.address,
    });
    expect(root.rollupStatus().lastCommitted).toEqual(1);

    val earlyFinalize = root.sendFinalizeBatch(sequencer.address, 1, { value: ton("0.05") });
    expect(earlyFinalize).toHaveFailedTx({
        from: sequencer.address,
        to: root.address,
        exitCode: Errors.ChallengeWindow,
    });

    testing.setNow(111);
    val finalize = root.sendFinalizeBatch(sequencer.address, 1, { value: ton("0.05") });
    expect(finalize).toHaveSuccessfulTx<FinalizeBatch>({
        from: sequencer.address,
        to: root.address,
    });
    expect(root.rollupStatus().lastFinalized).toEqual(1);
}

get fun `test vault deposit locks ton and emits deposit log`() {
    val deployer = testing.treasury("vault-deployer");
    val rollupRoot = testing.treasury("rollup-root");
    val user = testing.treasury("deposit-user");

    val vault = AssetVault.fromStorage({
        admin: deployer.address,
        rollupRoot: rollupRoot.address,
        wrappedGasMinter: createAddressNone(),
        paused: false,
        lockedTon: 0,
        releaseFailures: [],
    });
    val deploy = vault.deploy(deployer.address, { value: ton("0.1") });
    expect(deploy).toHaveSuccessfulDeploy({
        to: vault.address,
    });

    val deposit = vault.sendDepositTon(user.address, 77, ton("0.5"), 123, { value: ton("0.6") });
    expect(deposit).toHaveSuccessfulTx<DepositTon>({
        from: user.address,
        to: vault.address,
    });

    val status = vault.vaultStatus();
    expect(status.lockedTon).toEqual(ton("0.5"));
}

fun setupRoot(challengeWindowSec: uint32): (RollupRoot, Treasury, address) {
    val deployer = testing.treasury("root-deployer");
    val sequencer = testing.treasury("sequencer");
    val vault = testing.treasury("asset-vault");

    val root = RollupRoot.fromStorage({
        admin: deployer.address,
        sequencer: sequencer.address,
        assetVault: vault.address,
        challengeWindowSec,
        lastCommitted: 0,
        lastFinalized: 0,
        paused: false,
        commitments: [],
        claimedWithdrawals: [],
    });
    val deploy = root.deploy(deployer.address, { value: ton("0.1") });
    expect(deploy).toHaveSuccessfulDeploy({
        to: root.address,
    });

    return (root, sequencer, vault.address);
}
