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
    testing.setNow(100);
    val (root, sequencer, _) = setupRoot(10);

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
    val wrappedGasMinter = testing.treasury("wrapped-gas-minter");
    val user = testing.treasury("deposit-user");

    val vault = AssetVault.fromStorage({
        admin: deployer.address,
        rollupRoot: rollupRoot.address,
        wrappedGasMinter: wrappedGasMinter.address,
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

get fun `test fake withdrawal claim is rejected`() {
    testing.setNow(200);
    val (root, sequencer, _) = setupRoot(0);
    val attacker = testing.treasury("claim-attacker");

    val rootsA = BatchRootsA {
        prevStateRoot: 0,
        stateRoot: 21,
        txRoot: 22,
    };
    val rootsB = BatchRootsB {
        receiptRoot: 23,
        withdrawalRoot: 24,
        dataHash: 25,
    };

    root.sendCommitBatch(sequencer.address, 1, rootsA, rootsB, { value: ton("0.1") });
    root.sendFinalizeBatch(sequencer.address, 1, { value: ton("0.05") });

    val fakeWithdrawalId = 1234;
    val fakeLeaf = ReleaseAuthorized {
        withdrawalId: fakeWithdrawalId,
        assetId: 1,
        recipient: attacker.address,
        amount: ton("1"),
    }.toCell();
    val fakeProof = beginCell().storeUint(1, 1).endCell();

    val claim = root.sendClaimWithdrawal(
        attacker.address,
        1,
        fakeWithdrawalId,
        fakeLeaf,
        fakeProof,
        { value: ton("0.1") },
    );
    expect(claim).toHaveFailedTx({
        from: attacker.address,
        to: root.address,
        exitCode: Errors.BadWithdrawalProof,
    });
}

get fun `test fake jetton deposit notification is rejected`() {
    val deployer = testing.treasury("vault-deployer-2");
    val rollupRoot = testing.treasury("rollup-root-2");
    val wrappedGasMinter = testing.treasury("wrapped-gas-minter-2");
    val attacker = testing.treasury("jetton-attacker");
    val claimedSender = testing.treasury("claimed-jetton-sender");

    val vault = AssetVault.fromStorage({
        admin: deployer.address,
        rollupRoot: rollupRoot.address,
        wrappedGasMinter: wrappedGasMinter.address,
        paused: false,
        lockedTon: 0,
        releaseFailures: [],
    });
    vault.deploy(deployer.address, { value: ton("0.1") });

    val notification = vault.sendJettonTransferNotification(
        attacker.address,
        88,
        ton("10"),
        claimedSender.address,
        beginCell().storeUint(777, 256).endCell(),
        { value: ton("0.05") },
    );

    expect(notification).toHaveFailedTx({
        from: attacker.address,
        to: vault.address,
        exitCode: Errors.UnsupportedJettonDeposit,
    });
}

fun setupRoot(challengeWindowSec: uint32): (RollupRoot, Treasury, address) {
    val deployer = testing.treasury("root-deployer");
    val vault = testing.treasury("asset-vault");

    val root = RollupRoot.fromStorage({
        admin: deployer.address,
        sequencer: deployer.address,
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

    return (root, deployer, vault.address);
}
