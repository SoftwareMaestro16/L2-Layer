import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"

import "@shared/errors"
import "@shared/messages"
import "@shared/storage"
import "@wrappers/AssetVault.gen"
import "@wrappers/RollupRoot.gen"

get fun `test security rollup root rejects unauthorized commit`() {
    val (root, sequencer, attacker, _) = securityDeployRoot(30);

    val result = root.sendCommitBatch(
        attacker.address,
        1,
        securityRootsA(0, 11, 12),
        securityRootsB(13, 14, 15),
        { value: ton("0.1") },
    );

    expect(result).toHaveFailedTx({
        from: attacker.address,
        to: root.address,
        exitCode: Errors.Unauthorized,
    });
    expect(root.rollupStatus().sequencer).toEqual(sequencer.address);
    expect(root.rollupStatus().lastCommitted).toEqual(0);
}

get fun `test security rollup root enforces batch order and previous state root`() {
    val (root, sequencer, _, _) = securityDeployRoot(30);

    val skippedBatch = root.sendCommitBatch(
        sequencer.address,
        2,
        securityRootsA(0, 21, 22),
        securityRootsB(23, 24, 25),
        { value: ton("0.1") },
    );
    expect(skippedBatch).toHaveFailedTx({
        from: sequencer.address,
        to: root.address,
        exitCode: Errors.BadBatchNo,
    });

    val firstBatch = root.sendCommitBatch(
        sequencer.address,
        1,
        securityRootsA(0, 31, 32),
        securityRootsB(33, 34, 35),
        { value: ton("0.1") },
    );
    expect(firstBatch).toHaveSuccessfulTx<CommitBatch>({
        from: sequencer.address,
        to: root.address,
    });

    val staleRoot = root.sendCommitBatch(
        sequencer.address,
        2,
        securityRootsA(999, 41, 42),
        securityRootsB(43, 44, 45),
        { value: ton("0.1") },
    );
    expect(staleRoot).toHaveFailedTx({
        from: sequencer.address,
        to: root.address,
        exitCode: Errors.BadPrevRoot,
    });
    expect(root.rollupStatus().lastCommitted).toEqual(1);
}

get fun `test security rollup root rejects missing and duplicate finalization`() {
    testing.setNow(1000);
    val (root, sequencer, _, _) = securityDeployRoot(10);

    val missing = root.sendFinalizeBatch(sequencer.address, 1, { value: ton("0.05") });
    expect(missing).toHaveFailedTx({
        from: sequencer.address,
        to: root.address,
        exitCode: Errors.BatchNotFound,
    });

    root.sendCommitBatch(
        sequencer.address,
        1,
        securityRootsA(0, 51, 52),
        securityRootsB(53, 54, 55),
        { value: ton("0.1") },
    );

    testing.setNow(1011);
    val finalized = root.sendFinalizeBatch(sequencer.address, 1, { value: ton("0.05") });
    expect(finalized).toHaveSuccessfulTx<FinalizeBatch>({
        from: sequencer.address,
        to: root.address,
    });

    val duplicate = root.sendFinalizeBatch(sequencer.address, 1, { value: ton("0.05") });
    expect(duplicate).toHaveFailedTx({
        from: sequencer.address,
        to: root.address,
        exitCode: Errors.AlreadyFinalized,
    });
    expect(root.rollupStatus().lastFinalized).toEqual(1);
}

get fun `test security challenge gates finality and slashes sequencer bond`() {
    testing.setNow(3000);
    val (root, sequencer, challenger, _) = securityDeployRoot(10);

    val stake = root.sendAny(
        sequencer.address,
        StakeSequencerBond { amount: ton("1") }.toCell(),
        { value: ton("1.01") },
    );
    expect(stake).toHaveSuccessfulTx<StakeSequencerBond>({
        from: sequencer.address,
        to: root.address,
    });
    expect(root.bondStatus().sequencerBond).toEqual(ton("1"));

    root.sendCommitBatch(
        sequencer.address,
        1,
        securityRootsA(0, 61, 62),
        securityRootsB(63, 64, 65),
        { value: ton("0.1") },
    );

    val challenge = root.sendAny(
        challenger.address,
        ChallengeBatch {
            batchNo: 1,
            reason: CHALLENGE_REASON_INVALID_STATE_ROOT,
            evidenceHash: 999,
            challengerBond: MIN_CHALLENGE_BOND_NANOTON,
        }.toCell(),
        { value: ton("0.06") },
    );
    expect(challenge).toHaveSuccessfulTx<ChallengeBatch>({
        from: challenger.address,
        to: root.address,
    });

    val challengeStatus = root.batchChallenge(1);
    expect(challengeStatus.exists).toEqual(true);
    expect(challengeStatus.challenge.load().status).toEqual(CHALLENGE_STATUS_OPEN);

    testing.setNow(3011);
    val blockedFinalize = root.sendFinalizeBatch(sequencer.address, 1, { value: ton("0.05") });
    expect(blockedFinalize).toHaveFailedTx({
        from: sequencer.address,
        to: root.address,
        exitCode: Errors.ChallengeOpen,
    });

    val unauthorizedResolve = root.sendAny(
        challenger.address,
        ResolveChallenge {
            batchNo: 1,
            uphold: true,
            slashAmount: ton("0.4"),
        }.toCell(),
        { value: ton("0.05") },
    );
    expect(unauthorizedResolve).toHaveFailedTx({
        from: challenger.address,
        to: root.address,
        exitCode: Errors.Unauthorized,
    });

    val resolve = root.sendAny(
        sequencer.address,
        ResolveChallenge {
            batchNo: 1,
            uphold: true,
            slashAmount: ton("0.4"),
        }.toCell(),
        { value: ton("0.05") },
    );
    expect(resolve).toHaveSuccessfulTx<ResolveChallenge>({
        from: sequencer.address,
        to: root.address,
    });
    expect(root.bondStatus().sequencerBond).toEqual(ton("0.6"));
    expect(root.bondStatus().slashedBond).toEqual(ton("0.4"));
    expect(root.batchChallenge(1).challenge.load().status).toEqual(CHALLENGE_STATUS_UPHELD);

    val stillBlocked = root.sendFinalizeBatch(sequencer.address, 1, { value: ton("0.05") });
    expect(stillBlocked).toHaveFailedTx({
        from: sequencer.address,
        to: root.address,
        exitCode: Errors.ChallengeOpen,
    });
}

get fun `test security challenge requires evidence bond and unresolved batch`() {
    testing.setNow(4000);
    val (root, sequencer, challenger, _) = securityDeployRoot(10);

    root.sendCommitBatch(
        sequencer.address,
        1,
        securityRootsA(0, 71, 72),
        securityRootsB(73, 74, 75),
        { value: ton("0.1") },
    );

    val noEvidence = root.sendAny(
        challenger.address,
        ChallengeBatch {
            batchNo: 1,
            reason: CHALLENGE_REASON_DA_UNAVAILABLE,
            evidenceHash: 0,
            challengerBond: MIN_CHALLENGE_BOND_NANOTON,
        }.toCell(),
        { value: ton("0.06") },
    );
    expect(noEvidence).toHaveFailedTx({
        from: challenger.address,
        to: root.address,
        exitCode: Errors.BadChallenge,
    });

    val underBonded = root.sendAny(
        challenger.address,
        ChallengeBatch {
            batchNo: 1,
            reason: CHALLENGE_REASON_DA_UNAVAILABLE,
            evidenceHash: 123,
            challengerBond: MIN_CHALLENGE_BOND_NANOTON - 1,
        }.toCell(),
        { value: ton("0.06") },
    );
    expect(underBonded).toHaveFailedTx({
        from: challenger.address,
        to: root.address,
        exitCode: Errors.InsufficientBond,
    });

    val accepted = root.sendAny(
        challenger.address,
        ChallengeBatch {
            batchNo: 1,
            reason: CHALLENGE_REASON_DA_UNAVAILABLE,
            evidenceHash: 124,
            challengerBond: MIN_CHALLENGE_BOND_NANOTON,
        }.toCell(),
        { value: ton("0.06") },
    );
    expect(accepted).toHaveSuccessfulTx<ChallengeBatch>({
        from: challenger.address,
        to: root.address,
    });

    val rejected = root.sendAny(
        sequencer.address,
        ResolveChallenge {
            batchNo: 1,
            uphold: false,
            slashAmount: 0,
        }.toCell(),
        { value: ton("0.05") },
    );
    expect(rejected).toHaveSuccessfulTx<ResolveChallenge>({
        from: sequencer.address,
        to: root.address,
    });

    testing.setNow(4011);
    val finalize = root.sendFinalizeBatch(sequencer.address, 1, { value: ton("0.05") });
    expect(finalize).toHaveSuccessfulTx<FinalizeBatch>({
        from: sequencer.address,
        to: root.address,
    });

    val challengeAfterFinalize = root.sendAny(
        challenger.address,
        ChallengeBatch {
            batchNo: 1,
            reason: CHALLENGE_REASON_DA_UNAVAILABLE,
            evidenceHash: 125,
            challengerBond: MIN_CHALLENGE_BOND_NANOTON,
        }.toCell(),
        { value: ton("0.06") },
    );
    expect(challengeAfterFinalize).toHaveFailedTx({
        from: challenger.address,
        to: root.address,
        exitCode: Errors.AlreadyFinalized,
    });
}

get fun `test security rollup root ignores empty topup and rejects unknown non empty message`() {
    val (root, _, attacker, _) = securityDeployRoot(30);

    val emptyTopup = root.sendAny(
        attacker.address,
        beginCell().endCell(),
        { value: ton("0.01") },
    );
    expect(emptyTopup).toHaveSuccessfulTx({
        from: attacker.address,
        to: root.address,
    });

    val unknown = root.sendAny(
        attacker.address,
        beginCell().storeUint(0xdeadbeef, 32).endCell(),
        { value: ton("0.01") },
    );
    expect(unknown).toHaveFailedTx({
        from: attacker.address,
        to: root.address,
        exitCode: Errors.UnknownOpcode,
    });
}

get fun `test security vault rejects underfunded deposit and preserves accounting`() {
    val (vault, _, _, _, user) = securityDeployVault();

    val result = vault.sendDepositTon(
        user.address,
        100,
        ton("0.5"),
        123456,
        { value: ton("0.4") },
    );
    expect(result).toHaveFailedTx({
        from: user.address,
        to: vault.address,
        exitCode: Errors.BadDepositValue,
    });
    expect(vault.vaultStatus().lockedTon).toEqual(0);
}

get fun `test security vault rejects zero amount deposit`() {
    val (vault, _, _, _, user) = securityDeployVault();

    val result = vault.sendDepositTon(
        user.address,
        105,
        0,
        123456,
        { value: ton("0.1") },
    );
    expect(result).toHaveFailedTx({
        from: user.address,
        to: vault.address,
        exitCode: Errors.BadDepositValue,
    });
    expect(vault.vaultStatus().lockedTon).toEqual(0);
}

get fun `test security vault rejects zero l2 recipient deposit`() {
    val (vault, _, _, _, user) = securityDeployVault();

    val result = vault.sendDepositTon(
        user.address,
        106,
        ton("0.5"),
        0,
        { value: ton("0.5") },
    );
    expect(result).toHaveFailedTx({
        from: user.address,
        to: vault.address,
        exitCode: Errors.BadDepositValue,
    });
    expect(vault.vaultStatus().lockedTon).toEqual(0);
}

get fun `test security vault rejects unauthorized release and preserves locked ton`() {
    val (vault, _, rollupRoot, attacker, user) = securityDeployVault();

    val deposit = vault.sendDepositTon(user.address, 101, ton("1"), 777, { value: ton("1") });
    expect(deposit).toHaveSuccessfulTx<DepositTon>({
        from: user.address,
        to: vault.address,
    });

    val result = vault.sendReleaseAuthorized(
        attacker.address,
        501,
        1,
        user.address,
        ton("0.3"),
        { value: ton("0.05") },
    );
    expect(result).toHaveFailedTx({
        from: attacker.address,
        to: vault.address,
        exitCode: Errors.Unauthorized,
    });
    expect(vault.vaultStatus().rollupRoot).toEqual(rollupRoot.address);
    expect(vault.vaultStatus().lockedTon).toEqual(ton("1"));
}

get fun `test security vault authorized ton release reduces locked ton exactly`() {
    val (vault, _, rollupRoot, _, user) = securityDeployVault();

    vault.sendDepositTon(user.address, 102, ton("1"), 888, { value: ton("1") });

    val result = vault.sendReleaseAuthorized(
        rollupRoot.address,
        502,
        1,
        user.address,
        ton("0.3"),
        { value: ton("0.05") },
    );
    expect(result).toHaveSuccessfulTx<ReleaseAuthorized>({
        from: rollupRoot.address,
        to: vault.address,
    });
    expect(vault.vaultStatus().lockedTon).toEqual(ton("0.7"));
}

get fun `test security vault records recipient bounce and retry succeeds once`() {
    val (vault, _, rollupRoot, attacker, user) = securityDeployVault();

    vault.sendDepositTon(user.address, 104, ton("1"), 1000, { value: ton("1") });

    val release = ReleaseAuthorized {
        withdrawalId: 504,
        assetId: 1,
        recipient: user.address,
        amount: ton("0.3"),
    };
    val releaseToVault = createMessage({
        bounce: BounceMode.NoBounce,
        dest: vault.address,
        value: ton("0.05"),
        body: release,
    });
    testing.processSingleTraceStep(rollupRoot.address, releaseToVault);
    expect(vault.vaultStatus().lockedTon).toEqual(ton("0.7"));

    testing.processSingleTraceStep(
        user.address,
        securityRichReleaseBounce(vault.address, release, ton("0.3")),
    );

    expect(vault.vaultStatus().lockedTon).toEqual(ton("1"));
    val failed = vault.failedRelease(504);
    expect(failed.exists).toEqual(true);
    val failure = failed.failure.load();
    expect(failure.reason).toEqual(RELEASE_FAILURE_RECIPIENT_BOUNCE);
    expect(failure.retryCount).toEqual(0);

    val retry = vault.sendRetryRelease(attacker.address, 504, { value: ton("0.05") });
    expect(retry).toHaveSuccessfulTx<RetryRelease>({
        from: attacker.address,
        to: vault.address,
    });
    expect(vault.failedRelease(504).exists).toEqual(false);
    expect(vault.vaultStatus().lockedTon).toEqual(ton("0.7"));

    val duplicateRetry = vault.sendRetryRelease(attacker.address, 504, { value: ton("0.05") });
    expect(duplicateRetry).toHaveFailedTx({
        from: attacker.address,
        to: vault.address,
        exitCode: Errors.NoFailedWithdrawal,
    });
}

get fun `test security vault unsupported release does not debit locked ton`() {
    val (vault, _, rollupRoot, _, user) = securityDeployVault();

    vault.sendDepositTon(user.address, 103, ton("1"), 999, { value: ton("1") });

    val result = vault.sendReleaseAuthorized(
        rollupRoot.address,
        503,
        99,
        user.address,
        ton("0.3"),
        { value: ton("0.05") },
    );
    expect(result).toHaveSuccessfulTx<ReleaseAuthorized>({
        from: rollupRoot.address,
        to: vault.address,
    });
    expect(vault.vaultStatus().lockedTon).toEqual(ton("1"));

    val failed = vault.failedRelease(503);
    expect(failed.exists).toEqual(true);
    val failure = failed.failure.load();
    expect(failure.reason).toEqual(RELEASE_FAILURE_UNSUPPORTED_ASSET);

    val retry = vault.sendRetryRelease(user.address, 503, { value: ton("0.05") });
    expect(retry).toHaveFailedTx({
        from: user.address,
        to: vault.address,
        exitCode: Errors.UnsupportedReleaseAsset,
    });
    expect(vault.failedRelease(503).exists).toEqual(true);
}

get fun `test security vault rejects spoofed recipient bounce`() {
    val (vault, _, _, attacker, user) = securityDeployVault();

    val release = ReleaseAuthorized {
        withdrawalId: 505,
        assetId: 1,
        recipient: user.address,
        amount: ton("0.3"),
    };
    testing.processSingleTraceStep(
        attacker.address,
        securityRichReleaseBounce(vault.address, release, ton("0.3")),
    );

    expect(vault.failedRelease(505).exists).toEqual(false);
    expect(vault.vaultStatus().lockedTon).toEqual(0);
}

get fun `test security vault ignores empty topup and rejects unknown non empty message`() {
    val (vault, _, _, attacker, _) = securityDeployVault();

    val emptyTopup = vault.sendAny(
        attacker.address,
        beginCell().endCell(),
        { value: ton("0.01") },
    );
    expect(emptyTopup).toHaveSuccessfulTx({
        from: attacker.address,
        to: vault.address,
    });

    val unknown = vault.sendAny(
        attacker.address,
        beginCell().storeUint(0xdeadbeef, 32).endCell(),
        { value: ton("0.01") },
    );
    expect(unknown).toHaveFailedTx({
        from: attacker.address,
        to: vault.address,
        exitCode: Errors.UnknownOpcode,
    });
}

get fun `test security vault rejects unauthorized jetton asset registration`() {
    val (vault, deployer, _, attacker, user) = securityDeployVault();
    val jettonMaster = testing.treasury("security-jetton-master");

    val register = vault.sendRegisterJettonAsset(
        attacker.address,
        2,
        jettonMaster.address,
        user.address,
        9,
        { value: ton("0.05") },
    );
    expect(register).toHaveFailedTx({
        from: attacker.address,
        to: vault.address,
        exitCode: Errors.Unauthorized,
    });

    val authorized = vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        jettonMaster.address,
        user.address,
        9,
        { value: ton("0.05") },
    );
    expect(authorized).toHaveSuccessfulTx<RegisterJettonAsset>({
        from: deployer.address,
        to: vault.address,
    });
}

fun securityDeployRoot(challengeWindowSec: uint32): (RollupRoot, Treasury, Treasury, address) {
    val sequencer = testing.treasury("security-root-sequencer");
    val attacker = testing.treasury("security-root-attacker");
    val vault = testing.treasury("security-root-vault");

    val root = RollupRoot.fromStorage({
        admin: sequencer.address,
        sequencer: sequencer.address,
        assetVault: vault.address,
        challengeWindowSec,
        lastCommitted: 0,
        lastFinalized: 0,
        paused: false,
        security: emptyRollupSecurityState().toCell(),
        commitments: [],
        claimedWithdrawals: [],
        failedWithdrawals: [],
    });
    val deploy = root.deploy(sequencer.address, { value: ton("0.1") });
    expect(deploy).toHaveSuccessfulDeploy({
        to: root.address,
    });

    return (root, sequencer, attacker, vault.address);
}

fun securityDeployVault(): (AssetVault, Treasury, Treasury, Treasury, Treasury) {
    val deployer = testing.treasury("security-vault-deployer");
    val rollupRoot = testing.treasury("security-vault-rollup-root");
    val attacker = testing.treasury("security-vault-attacker");
    val user = testing.treasury("security-vault-user");
    val wrappedGasMinter = testing.treasury("security-vault-wrapped-gas-minter");

    val vault = AssetVault.fromStorage({
        admin: deployer.address,
        rollupRoot: rollupRoot.address,
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
    val deploy = vault.deploy(deployer.address, { value: ton("0.1") });
    expect(deploy).toHaveSuccessfulDeploy({
        to: vault.address,
    });

    return (vault, deployer, rollupRoot, attacker, user);
}

fun securityRichReleaseBounce(
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

fun securityRootsA(prevStateRoot: uint256, stateRoot: uint256, txRoot: uint256): BatchRootsA {
    return BatchRootsA { prevStateRoot, stateRoot, txRoot };
}

fun securityRootsB(receiptRoot: uint256, withdrawalRoot: uint256, dataHash: uint256): BatchRootsB {
    return BatchRootsB { receiptRoot, withdrawalRoot, dataHash };
}
