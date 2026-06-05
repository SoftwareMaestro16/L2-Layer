import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"

import "@shared/errors"
import "@shared/messages"
import "@shared/storage"
import "@wrappers/AssetVault.gen"

get fun `test hardening jetton registry rejects zero master and wallet`() {
    val (
        vault,
        deployer,
        _,
        _,
        _,
        jettonMaster,
        jettonWallet,
        _,
    ) = setupHardeningJettonVault();

    val zeroMaster = vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        unlinkedAssetVaultAddress(),
        jettonWallet.address,
        9,
        { value: ton("0.05") },
    );
    expect(zeroMaster).toHaveFailedTx({
        from: deployer.address,
        to: vault.address,
        exitCode: Errors.BadAssetConfig,
    });

    val zeroWallet = vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        jettonMaster.address,
        unlinkedAssetVaultAddress(),
        9,
        { value: ton("0.05") },
    );
    expect(zeroWallet).toHaveFailedTx({
        from: deployer.address,
        to: vault.address,
        exitCode: Errors.BadAssetConfig,
    });
}

get fun `test hardening jetton registry rejects wallet reused by another asset`() {
    val (
        vault,
        deployer,
        _,
        _,
        _,
        jettonMaster,
        jettonWallet,
        _,
    ) = setupHardeningJettonVault();

    val first = vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        jettonMaster.address,
        jettonWallet.address,
        9,
        { value: ton("0.05") },
    );
    expect(first).toHaveSuccessfulTx<RegisterJettonAsset>({
        from: deployer.address,
        to: vault.address,
    });

    val reusedWallet = vault.sendRegisterJettonAsset(
        deployer.address,
        3,
        jettonMaster.address,
        jettonWallet.address,
        9,
        { value: ton("0.05") },
    );
    expect(reusedWallet).toHaveFailedTx({
        from: deployer.address,
        to: vault.address,
        exitCode: Errors.BadJettonWallet,
    });
}

get fun `test hardening jetton registry keeps master and decimals immutable`() {
    val (
        vault,
        deployer,
        _,
        attacker,
        _,
        jettonMaster,
        firstWallet,
        secondWallet,
    ) = setupHardeningJettonVault();

    vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        jettonMaster.address,
        firstWallet.address,
        9,
        { value: ton("0.05") },
    );

    val changedMaster = vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        attacker.address,
        secondWallet.address,
        9,
        { value: ton("0.05") },
    );
    expect(changedMaster).toHaveFailedTx({
        from: deployer.address,
        to: vault.address,
        exitCode: Errors.BadAssetConfig,
    });

    val changedDecimals = vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        jettonMaster.address,
        secondWallet.address,
        8,
        { value: ton("0.05") },
    );
    expect(changedDecimals).toHaveFailedTx({
        from: deployer.address,
        to: vault.address,
        exitCode: Errors.BadAssetConfig,
    });

    val rotatedWallet = vault.sendRegisterJettonAsset(
        deployer.address,
        2,
        jettonMaster.address,
        secondWallet.address,
        9,
        { value: ton("0.05") },
    );
    expect(rotatedWallet).toHaveSuccessfulTx<RegisterJettonAsset>({
        from: deployer.address,
        to: vault.address,
    });
}

fun setupHardeningJettonVault(): (
    AssetVault,
    Treasury,
    Treasury,
    Treasury,
    Treasury,
    Treasury,
    Treasury,
    Treasury,
) {
    val deployer = testing.treasury("jetton-hardening-deployer");
    val rollupRoot = testing.treasury("jetton-hardening-rollup-root");
    val user = testing.treasury("jetton-hardening-user");
    val attacker = testing.treasury("jetton-hardening-attacker");
    val jettonMaster = testing.treasury("jetton-hardening-master");
    val firstWallet = testing.treasury("jetton-hardening-wallet-1");
    val secondWallet = testing.treasury("jetton-hardening-wallet-2");
    val wrappedGasMinter = testing.treasury("jetton-hardening-gas-minter");

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
        jettonMaster,
        firstWallet,
        secondWallet,
    );
}
