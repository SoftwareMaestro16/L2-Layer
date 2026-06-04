import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"

import "@shared/crypto"
import "@shared/errors"
import "@shared/messages"
import "@shared/proofs"
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

get fun `test valid finalized withdrawal claim is accepted and replay is rejected`() {
    testing.setNow(200);
    val (root, sequencer, _) = setupRoot(0);
    val user = testing.treasury("claim-user");

    val withdrawalId = 1234;
    val leaf = releaseLeaf(withdrawalId, 1, user.address, ton("0.3"));
    commitAndFinalizeWithdrawalRoot(root, sequencer, 1, cellReprHash(leaf));

    val claim = root.sendClaimWithdrawal(
        user.address,
        1,
        withdrawalId,
        leaf,
        emptyWithdrawalProof(),
        { value: ton("0.1") },
    );
    expect(claim).toHaveSuccessfulTx<ClaimWithdrawal>({
        from: user.address,
        to: root.address,
    });

    val replay = root.sendClaimWithdrawal(
        user.address,
        1,
        withdrawalId,
        leaf,
        emptyWithdrawalProof(),
        { value: ton("0.1") },
    );
    expect(replay).toHaveFailedTx({
        from: user.address,
        to: root.address,
        exitCode: Errors.AlreadyClaimed,
    });
}

get fun `test bounced vault release is recorded and permissionless root retry clears failure`() {
    testing.setNow(205);
    val (root, sequencer, vaultAddress) = setupRoot(0);
    val user = testing.treasury("root-retry-user");
    val retryOperator = testing.treasury("root-retry-operator");

    val withdrawalId = 9234;
    val leaf = releaseLeaf(withdrawalId, 1, user.address, ton("0.3"));
    commitAndFinalizeWithdrawalRoot(root, sequencer, 1, cellReprHash(leaf));

    val claimMsg = createMessage({
        bounce: BounceMode.NoBounce,
        dest: root.address,
        value: ton("0.1"),
        body: ClaimWithdrawal {
            batchNo: 1,
            withdrawalId,
            withdrawalLeaf: leaf,
            merkleProof: emptyWithdrawalProof(),
        },
    });
    testing.processSingleTraceStep(user.address, claimMsg);

    val release = ReleaseAuthorized {
        withdrawalId,
        assetId: 1,
        recipient: user.address,
        amount: ton("0.3"),
    };
    testing.processSingleTraceStep(
        vaultAddress,
        rootRichReleaseBounce(root.address, release, ton("0.08")),
    );

    val failed = root.failedWithdrawal(withdrawalId);
    expect(failed.exists).toEqual(true);
    val failure = failed.failure.load();
    expect(failure.reason).toEqual(RELEASE_FAILURE_VAULT_BOUNCE);
    expect(failure.retryCount).toEqual(0);

    val retry = root.sendRetryWithdrawal(
        retryOperator.address,
        withdrawalId,
        { value: ton("0.1") },
    );
    expect(retry).toHaveSuccessfulTx<RetryWithdrawal>({
        from: retryOperator.address,
        to: root.address,
    });
    expect(root.failedWithdrawal(withdrawalId).exists).toEqual(false);

    val duplicateRetry = root.sendRetryWithdrawal(
        retryOperator.address,
        withdrawalId,
        { value: ton("0.1") },
    );
    expect(duplicateRetry).toHaveFailedTx({
        from: retryOperator.address,
        to: root.address,
        exitCode: Errors.NoFailedWithdrawal,
    });
}

get fun `test bounced vault release without proven claim is rejected`() {
    testing.setNow(206);
    val (root, _, vaultAddress) = setupRoot(0);
    val user = testing.treasury("root-spoof-user");

    val release = ReleaseAuthorized {
        withdrawalId: 9345,
        assetId: 1,
        recipient: user.address,
        amount: ton("0.3"),
    };
    testing.processSingleTraceStep(
        vaultAddress,
        rootRichReleaseBounce(root.address, release, ton("0.08")),
    );

    expect(root.failedWithdrawal(9345).exists).toEqual(false);
}

get fun `test finalized withdrawal claim rejects wrong root`() {
    testing.setNow(210);
    val (root, sequencer, _) = setupRoot(0);
    val attacker = testing.treasury("claim-attacker");

    val fakeWithdrawalId = 1234;
    val fakeLeaf = releaseLeaf(fakeWithdrawalId, 1, attacker.address, ton("1"));
    commitAndFinalizeWithdrawalRoot(root, sequencer, 1, 24);

    val claim = root.sendClaimWithdrawal(
        attacker.address,
        1,
        fakeWithdrawalId,
        fakeLeaf,
        emptyWithdrawalProof(),
        { value: ton("0.1") },
    );
    expect(claim).toHaveFailedTx({
        from: attacker.address,
        to: root.address,
        exitCode: Errors.BadWithdrawalProof,
    });
}

get fun `test withdrawal claim before finalization is rejected`() {
    testing.setNow(220);
    val (root, sequencer, _) = setupRoot(100);
    val user = testing.treasury("early-claim-user");

    val withdrawalId = 2234;
    val leaf = releaseLeaf(withdrawalId, 1, user.address, ton("0.2"));
    val rootsA = BatchRootsA {
        prevStateRoot: 0,
        stateRoot: 121,
        txRoot: 122,
    };
    val rootsB = BatchRootsB {
        receiptRoot: 123,
        withdrawalRoot: cellReprHash(leaf),
        dataHash: 125,
    };
    root.sendCommitBatch(sequencer.address, 1, rootsA, rootsB, { value: ton("0.1") });

    val claim = root.sendClaimWithdrawal(
        user.address,
        1,
        withdrawalId,
        leaf,
        emptyWithdrawalProof(),
        { value: ton("0.1") },
    );
    expect(claim).toHaveFailedTx({
        from: user.address,
        to: root.address,
        exitCode: Errors.NotFinalized,
    });
}

get fun `test withdrawal claim rejects mismatched withdrawal id in leaf`() {
    testing.setNow(230);
    val (root, sequencer, _) = setupRoot(0);
    val user = testing.treasury("mismatch-claim-user");

    val leaf = releaseLeaf(3333, 1, user.address, ton("0.2"));
    commitAndFinalizeWithdrawalRoot(root, sequencer, 1, cellReprHash(leaf));

    val claim = root.sendClaimWithdrawal(
        user.address,
        1,
        4444,
        leaf,
        emptyWithdrawalProof(),
        { value: ton("0.1") },
    );
    expect(claim).toHaveFailedTx({
        from: user.address,
        to: root.address,
        exitCode: Errors.BadWithdrawalProof,
    });
}

get fun `test withdrawal claim rejects corrupted sibling order`() {
    testing.setNow(240);
    val (root, sequencer, _) = setupRoot(0);
    val user = testing.treasury("sibling-claim-user");

    val leftLeaf = releaseLeaf(5555, 1, user.address, ton("0.2"));
    val rightLeaf = releaseLeaf(6666, 1, user.address, ton("0.4"));
    val rootHash = hashWithdrawalNode(cellReprHash(leftLeaf), cellReprHash(rightLeaf));
    commitAndFinalizeWithdrawalRoot(root, sequencer, 1, rootHash);

    val claim = root.sendClaimWithdrawal(
        user.address,
        1,
        5555,
        leftLeaf,
        oneSiblingWithdrawalProof(1, cellReprHash(rightLeaf)),
        { value: ton("0.1") },
    );
    expect(claim).toHaveFailedTx({
        from: user.address,
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

fun releaseLeaf(withdrawalId: uint256, assetId: uint32, recipient: address, amount: coins): cell {
    return ReleaseAuthorized { withdrawalId, assetId, recipient, amount }.toCell();
}

fun emptyWithdrawalProof(): cell {
    return WithdrawalMerkleProof {
        leafIndex: 0,
        siblingsCount: 0,
        first: null,
    }.toCell();
}

fun oneSiblingWithdrawalProof(leafIndex: uint64, sibling: uint256): cell {
    val chunk = WithdrawalProofChunk {
        count: 1,
        sibling0: sibling,
        sibling1: 0,
        sibling2: 0,
        next: null,
    }.toCell();
    return WithdrawalMerkleProof {
        leafIndex,
        siblingsCount: 1,
        first: chunk,
    }.toCell();
}

fun rootRichReleaseBounce(
    dest: address,
    release: ReleaseAuthorized,
    valueCoins: coins,
): OutMessage {
    return createMessage({
        bounce: BounceMode.NoBounce,
        dest,
        value: ton("0.02"),
        body: RichBounceBody {
            originalBody: release.toCell(),
            originalInfo: RichBounceOriginalMsgInfo {
                valueCoins,
                valueExtra: [],
                createdLt: 0,
                createdAt: blockchain.now(),
            }.toCell(),
            bouncedByPhase: 1,
            exitCode: Errors.Paused,
            computePhase: null,
        },
    }).bounced();
}

fun commitAndFinalizeWithdrawalRoot(
    root: RollupRoot,
    sequencer: Treasury,
    batchNo: uint64,
    withdrawalRoot: uint256,
) {
    val rootsA = BatchRootsA {
        prevStateRoot: 0,
        stateRoot: 21,
        txRoot: 22,
    };
    val rootsB = BatchRootsB {
        receiptRoot: 23,
        withdrawalRoot,
        dataHash: 25,
    };
    root.sendCommitBatch(sequencer.address, batchNo, rootsA, rootsB, { value: ton("0.1") });
    root.sendFinalizeBatch(sequencer.address, batchNo, { value: ton("0.05") });
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
        failedWithdrawals: [],
    });
    val deploy = root.deploy(deployer.address, { value: ton("0.1") });
    expect(deploy).toHaveSuccessfulDeploy({
        to: root.address,
    });

    return (root, deployer, vault.address);
}
