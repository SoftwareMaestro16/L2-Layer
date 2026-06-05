import "@shared/errors"
import "@shared/messages"
import "@shared/storage"

fun handleStakeSequencerBond(senderAddress: address, valueCoins: coins, msg: StakeSequencerBond) {
    var storage = lazy RollupStorage.load();
    assert (!storage.paused) throw Errors.Paused;
    assert (senderAddress == storage.sequencer) throw Errors.Unauthorized;
    assert (msg.amount > 0) throw Errors.InsufficientBond;
    assert (valueCoins >= msg.amount) throw Errors.InsufficientBond;

    var security = storage.security.load();
    security.sequencerBond += msg.amount;
    storage.security = security.toCell();
    storage.save();
}

fun handleChallengeBatch(senderAddress: address, valueCoins: coins, msg: ChallengeBatch) {
    var storage = lazy RollupStorage.load();
    assert (!storage.paused) throw Errors.Paused;
    assert (msg.evidenceHash != 0) throw Errors.BadChallenge;
    assert (msg.reason > 0) throw Errors.BadChallenge;
    assert (msg.challengerBond >= MIN_CHALLENGE_BOND_NANOTON) throw Errors.InsufficientBond;
    assert (valueCoins >= msg.challengerBond) throw Errors.InsufficientBond;

    val found = storage.commitments.get(msg.batchNo);
    assert (found.isFound) throw Errors.BatchNotFound;
    assert (!found.loadValue().load().finalized) throw Errors.AlreadyFinalized;

    var security = storage.security.load();
    val existing = security.challenges.get(msg.batchNo);
    assert (!existing.isFound) throw Errors.ChallengeAlreadyExists;

    security
        .challenges
        .set(
            msg.batchNo,
            BatchChallenge {
                batchNo: msg.batchNo,
                challenger: senderAddress,
                reason: msg.reason,
                evidenceHash: msg.evidenceHash,
                openedAt: blockchain.now(),
                status: CHALLENGE_STATUS_OPEN,
                challengerBond: msg.challengerBond,
                slashedAmount: 0,
            }.toCell(),
        );
    storage.security = security.toCell();
    storage.save();
}

fun handleResolveChallenge(senderAddress: address, msg: ResolveChallenge) {
    var storage = lazy RollupStorage.load();
    assert (!storage.paused) throw Errors.Paused;
    assert (senderAddress == storage.admin) throw Errors.Unauthorized;

    var security = storage.security.load();
    val found = security.challenges.get(msg.batchNo);
    assert (found.isFound) throw Errors.ChallengeNotFound;

    var challenge = found.loadValue().load();
    assert (challenge.status == CHALLENGE_STATUS_OPEN) throw Errors.ChallengeNotFound;

    if (msg.uphold) {
        assert (msg.slashAmount > 0) throw Errors.BadChallenge;
        assert (security.sequencerBond >= msg.slashAmount) throw Errors.InsufficientBond;
        security.sequencerBond -= msg.slashAmount;
        security.slashedBond += msg.slashAmount;
        challenge.status = CHALLENGE_STATUS_UPHELD;
        challenge.slashedAmount = msg.slashAmount;
    } else {
        assert (msg.slashAmount == 0) throw Errors.BadChallenge;
        challenge.status = CHALLENGE_STATUS_REJECTED;
        challenge.slashedAmount = 0;
    }

    security.challenges.set(msg.batchNo, challenge.toCell());
    storage.security = security.toCell();
    storage.save();
}
