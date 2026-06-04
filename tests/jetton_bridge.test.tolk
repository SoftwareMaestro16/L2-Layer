import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"

import "@shared/errors"
import "@shared/messages"
import "@shared/storage"
import "@wrappers/AssetVault.gen"

get fun `test valid jetton deposit notification is accepted after registry`() {
    val deployer = testing.treasury("vault-deployer-2");
    val rollupRoot = testing.treasury("rollup-root-2");
    val wrappedGasMinter = testing.treasury("wrapped-gas-minter-2");
    val jettonMaster = testing.treasury("jetton-master-2");
    val jettonWallet = testing.treasury("jetton-wallet-2");
    val claimedSender = testing.treasury("claimed-jetton-sender");

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
    vault.deploy(deployer.address, { value: ton("0.1") });

    val register = vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        jettonMaster.address,
        jettonWallet.address,
        9,
        { value: ton("0.05") },
    );
    expect(register).toHaveSuccessfulTx<RegisterJettonAsset>({
        from: deployer.address,
        to: vault.address,
    });
    expect(vault.jettonAsset(2).exists).toEqual(true);

    val notification = vault.sendAny(
        jettonWallet.address,
        jettonNotificationRefBody(88, ton("10"), claimedSender.address, 777),
        { value: ton("0.05") },
    );
    expect(notification).toHaveSuccessfulTx<JettonTransferNotification>({
        from: jettonWallet.address,
        to: vault.address,
    });
}

get fun `test canonical inline jetton deposit notification is accepted`() {
    val (
        vault,
        deployer,
        _,
        _,
        _,
        claimedSender,
        jettonMaster,
        jettonWallet,
    ) = setupJettonVault();

    vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        jettonMaster.address,
        jettonWallet.address,
        9,
        { value: ton("0.05") },
    );

    val notification = vault.sendAny(
        jettonWallet.address,
        jettonNotificationInlineBody(89, ton("10"), claimedSender.address, 777),
        { value: ton("0.05") },
    );
    expect(notification).toHaveSuccessfulTx<JettonTransferNotification>({
        from: jettonWallet.address,
        to: vault.address,
    });
}

get fun `test non canonical jetton notification shortcut is rejected`() {
    val (
        vault,
        deployer,
        _,
        _,
        _,
        claimedSender,
        jettonMaster,
        jettonWallet,
    ) = setupJettonVault();

    vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        jettonMaster.address,
        jettonWallet.address,
        9,
        { value: ton("0.05") },
    );

    val notification = vault.sendJettonTransferNotification(
        jettonWallet.address,
        90,
        ton("10"),
        claimedSender.address,
        beginCell().storeUint(777, 256).endCell(),
        { value: ton("0.05") },
    );
    expect(notification).toHaveFailedTx({
        from: jettonWallet.address,
        to: vault.address,
        exitCode: Errors.BadForwardPayload,
    });
}

get fun `test fake jetton deposit notification is rejected`() {
    val (
        vault,
        deployer,
        _,
        _,
        attacker,
        claimedSender,
        jettonMaster,
        jettonWallet,
    ) = setupJettonVault();

    vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        jettonMaster.address,
        jettonWallet.address,
        9,
        { value: ton("0.05") },
    );

    val notification = vault.sendAny(
        attacker.address,
        jettonNotificationRefBody(91, ton("10"), claimedSender.address, 777),
        { value: ton("0.05") },
    );

    expect(notification).toHaveFailedTx({
        from: attacker.address,
        to: vault.address,
        exitCode: Errors.BadJettonWallet,
    });
}

get fun `test malformed jetton forward payload is rejected`() {
    val (
        vault,
        deployer,
        _,
        _,
        _,
        claimedSender,
        jettonMaster,
        jettonWallet,
    ) = setupJettonVault();

    vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        jettonMaster.address,
        jettonWallet.address,
        9,
        { value: ton("0.05") },
    );

    val notification = vault.sendAny(
        jettonWallet.address,
        jettonNotificationRefCellBody(
            92,
            ton("10"),
            claimedSender.address,
            beginCell().storeUint(777, 256).storeBool(true).endCell(),
        ),
        { value: ton("0.05") },
    );

    expect(notification).toHaveFailedTx({
        from: jettonWallet.address,
        to: vault.address,
        exitCode: Errors.BadForwardPayload,
    });
}

get fun `test jetton forward payload with ref tail is rejected`() {
    val (
        vault,
        deployer,
        _,
        _,
        _,
        claimedSender,
        jettonMaster,
        jettonWallet,
    ) = setupJettonVault();

    vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        jettonMaster.address,
        jettonWallet.address,
        9,
        { value: ton("0.05") },
    );

    val notification = vault.sendAny(
        jettonWallet.address,
        jettonNotificationRefWithTailBody(
            93,
            ton("10"),
            claimedSender.address,
            beginCell().storeUint(777, 256).endCell(),
        ),
        { value: ton("0.05") },
    );

    expect(notification).toHaveFailedTx({
        from: jettonWallet.address,
        to: vault.address,
        exitCode: Errors.BadForwardPayload,
    });
}

get fun `test zero amount jetton deposit is rejected`() {
    val (
        vault,
        deployer,
        _,
        _,
        _,
        claimedSender,
        jettonMaster,
        jettonWallet,
    ) = setupJettonVault();

    vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        jettonMaster.address,
        jettonWallet.address,
        9,
        { value: ton("0.05") },
    );

    val notification = vault.sendAny(
        jettonWallet.address,
        jettonNotificationRefBody(94, 0, claimedSender.address, 777),
        { value: ton("0.05") },
    );

    expect(notification).toHaveFailedTx({
        from: jettonWallet.address,
        to: vault.address,
        exitCode: Errors.BadJettonAmount,
    });
}

get fun `test jetton registry update removes previous trusted wallet`() {
    val (
        vault,
        deployer,
        _,
        user,
        _,
        claimedSender,
        jettonMaster,
        oldWallet,
    ) = setupJettonVault();

    vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        jettonMaster.address,
        oldWallet.address,
        9,
        { value: ton("0.05") },
    );
    vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        jettonMaster.address,
        claimedSender.address,
        9,
        { value: ton("0.05") },
    );

    val oldNotification = vault.sendAny(
        oldWallet.address,
        jettonNotificationRefBody(95, ton("1"), user.address, 777),
        { value: ton("0.05") },
    );
    expect(oldNotification).toHaveFailedTx({
        from: oldWallet.address,
        to: vault.address,
        exitCode: Errors.BadJettonWallet,
    });

    val newNotification = vault.sendAny(
        claimedSender.address,
        jettonNotificationRefBody(96, ton("1"), user.address, 777),
        { value: ton("0.05") },
    );
    expect(newNotification).toHaveSuccessfulTx<JettonTransferNotification>({
        from: claimedSender.address,
        to: vault.address,
    });
}

get fun `test jetton asset registration rejects invalid decimals`() {
    val (
        vault,
        deployer,
        _,
        _,
        _,
        _,
        jettonMaster,
        jettonWallet,
    ) = setupJettonVault();

    val register = vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        jettonMaster.address,
        jettonWallet.address,
        19,
        { value: ton("0.05") },
    );

    expect(register).toHaveFailedTx({
        from: deployer.address,
        to: vault.address,
        exitCode: Errors.BadAssetDecimals,
    });
}

get fun `test authorized jetton release routes through registered wallet`() {
    val (
        vault,
        deployer,
        rollupRoot,
        user,
        _,
        _,
        jettonMaster,
        jettonWallet,
    ) = setupJettonVault();

    vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        jettonMaster.address,
        jettonWallet.address,
        9,
        { value: ton("0.05") },
    );

    val release = vault.sendReleaseAuthorized(
        rollupRoot.address,
        610,
        2,
        user.address,
        ton("4"),
        { value: ton("0.1") },
    );
    expect(release).toHaveSuccessfulTx<ReleaseAuthorized>({
        from: rollupRoot.address,
        to: vault.address,
    });
    expect(vault.failedRelease(610).exists).toEqual(false);
}

fun jettonNotificationRefBody(
    queryId: uint64,
    amount: coins,
    sender: address,
    l2Recipient: uint256,
): cell {
    return jettonNotificationRefCellBody(
        queryId,
        amount,
        sender,
        beginCell().storeUint(l2Recipient, 256).endCell(),
    );
}

fun jettonNotificationRefCellBody(
    queryId: uint64,
    amount: coins,
    sender: address,
    payload: cell,
): cell {
    return beginCell()
        .storeUint(0x7362d09c, 32)
        .storeUint(queryId, 64)
        .storeCoins(amount)
        .storeAddress(sender)
        .storeBool(true)
        .storeRef(payload)
        .endCell();
}

fun jettonNotificationRefWithTailBody(
    queryId: uint64,
    amount: coins,
    sender: address,
    payload: cell,
): cell {
    return beginCell()
        .storeUint(0x7362d09c, 32)
        .storeUint(queryId, 64)
        .storeCoins(amount)
        .storeAddress(sender)
        .storeBool(true)
        .storeRef(payload)
        .storeBool(true)
        .endCell();
}

fun jettonNotificationInlineBody(
    queryId: uint64,
    amount: coins,
    sender: address,
    l2Recipient: uint256,
): cell {
    return beginCell()
        .storeUint(0x7362d09c, 32)
        .storeUint(queryId, 64)
        .storeCoins(amount)
        .storeAddress(sender)
        .storeBool(false)
        .storeUint(l2Recipient, 256)
        .endCell();
}

fun setupJettonVault(): (
    AssetVault,
    Treasury,
    Treasury,
    Treasury,
    Treasury,
    Treasury,
    Treasury,
    Treasury,
) {
    val deployer = testing.treasury("jetton-vault-deployer");
    val rollupRoot = testing.treasury("jetton-vault-rollup-root");
    val user = testing.treasury("jetton-vault-user");
    val attacker = testing.treasury("jetton-vault-attacker");
    val claimedSender = testing.treasury("jetton-vault-sender");
    val jettonMaster = testing.treasury("jetton-vault-master");
    val jettonWallet = testing.treasury("jetton-vault-wallet");
    val wrappedGasMinter = testing.treasury("jetton-vault-wrapped-gas-minter");

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

    return (
        vault,
        deployer,
        rollupRoot,
        user,
        attacker,
        claimedSender,
        jettonMaster,
        jettonWallet,
    );
}
